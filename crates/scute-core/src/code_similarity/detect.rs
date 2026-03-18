use std::collections::HashMap;

use super::tree::Token;

/// A group of code regions that share the same normalized token sequence.
///
/// Invariant: `occurrences` always has at least 2 entries (a clone requires
/// at least two locations).
#[derive(Debug, Clone, PartialEq)]
pub struct CloneGroup {
    pub token_count: usize,
    pub occurrences: Vec<Occurrence>,
}

/// A single occurrence of a clone within a source.
///
/// Identifies exactly which tokens belong to this occurrence via
/// `source_idx` (index into the sources array) and `token_start`
/// (index into that source's token sequence). The token range is
/// `token_start..token_start + group.token_count`.
#[derive(Debug, Clone, PartialEq)]
pub struct Occurrence {
    pub source_idx: usize,
    pub token_start: usize,
}

struct TokenSequence {
    concat: Vec<usize>,
    /// Maps each position in `concat` to `(file_index, token_index)`.
    /// `None` for sentinel positions.
    pos_map: Vec<Option<(usize, usize)>>,
}

/// Detect clone groups across the given token sequences.
///
/// Builds a generalized suffix array over the concatenated normalized
/// token texts, then walks the LCP array to find all maximal repeated
/// regions of at least `min_tokens` length.
///
/// Occurrences reference sources by index and carry token positions.
/// Each position maps directly to the `Token` it came from.
#[must_use]
pub fn detect_clones(sources: &[Vec<Token>], min_tokens: usize) -> Vec<CloneGroup> {
    if sources.is_empty() || min_tokens == 0 {
        return vec![];
    }

    let seq = build_token_sequence(sources);
    if seq.concat.len() < 2 {
        return vec![];
    }

    let sa = build_suffix_array(&seq.concat);
    let lcp = build_lcp_array(&seq.concat, &sa);
    let intervals = extract_lcp_intervals(&sa, &lcp, min_tokens);

    let groups = intervals_to_groups(&intervals, &sa, &seq.pos_map, sources);
    filter_maximal_groups(groups)
}

/// Concatenate all token streams with unique sentinels between them,
/// mapping each position back to its source and token index.
fn build_token_sequence(sources: &[Vec<Token>]) -> TokenSequence {
    // Real token IDs start at 0. Sentinels use the high end of usize
    // (usize::MAX, usize::MAX-1, …) so they never collide with real IDs.
    let mut vocab: HashMap<&str, usize> = HashMap::new();
    let mut concat: Vec<usize> = Vec::new();
    let mut pos_map: Vec<Option<(usize, usize)>> = Vec::new();

    for (source_idx, tokens) in sources.iter().enumerate() {
        for (tok_idx, tok) in tokens.iter().enumerate() {
            let next_id = vocab.len();
            let id = *vocab.entry(tok.text.as_str()).or_insert(next_id);
            concat.push(id);
            pos_map.push(Some((source_idx, tok_idx)));
        }
        concat.push(usize::MAX - source_idx); // unique sentinel per source
        pos_map.push(None);
    }

    TokenSequence { concat, pos_map }
}

/// Convert LCP intervals into clone groups with token-level positions.
fn intervals_to_groups(
    intervals: &[(usize, usize, usize)],
    sa: &[usize],
    pos_map: &[Option<(usize, usize)>],
    sources: &[Vec<Token>],
) -> Vec<CloneGroup> {
    intervals
        .iter()
        .filter_map(|&(depth, left, right)| {
            let occurrences = collect_occurrences(&sa[left..=right], pos_map, sources, depth);
            (occurrences.len() >= 2).then_some(CloneGroup {
                token_count: depth,
                occurrences,
            })
        })
        .collect()
}

/// For a single LCP interval, collect all valid occurrences from the suffix array.
fn collect_occurrences(
    sa_slice: &[usize],
    pos_map: &[Option<(usize, usize)>],
    sources: &[Vec<Token>],
    depth: usize,
) -> Vec<Occurrence> {
    let mut occurrences: Vec<Occurrence> = sa_slice
        .iter()
        .filter_map(|&pos| {
            let (source_idx, tok_idx) = pos_map[pos]?;
            (tok_idx + depth <= sources[source_idx].len()).then_some(Occurrence {
                source_idx,
                token_start: tok_idx,
            })
        })
        .collect();

    occurrences.sort_by(|a, b| {
        a.source_idx
            .cmp(&b.source_idx)
            .then(a.token_start.cmp(&b.token_start))
    });
    occurrences.dedup();
    occurrences
}

/// Check if every occurrence in `candidate` is contained within
/// some occurrence of `accepted` (token-range containment).
fn is_subsumed_by(candidate: &CloneGroup, accepted: &[CloneGroup]) -> bool {
    accepted.iter().any(|prev| {
        candidate.occurrences.iter().all(|occ| {
            let occ_end = occ.token_start + candidate.token_count;
            prev.occurrences.iter().any(|p| {
                p.source_idx == occ.source_idx
                    && p.token_start <= occ.token_start
                    && p.token_start + prev.token_count >= occ_end
            })
        })
    })
}

/// Keep only maximal matches: discard groups where every occurrence is
/// spatially contained within an already-accepted longer group.
fn filter_maximal_groups(mut groups: Vec<CloneGroup>) -> Vec<CloneGroup> {
    // Deterministic output: longest matches first, then by occurrence count
    groups.sort_by(|a, b| {
        b.token_count
            .cmp(&a.token_count)
            .then(a.occurrences.len().cmp(&b.occurrences.len()))
    });

    let mut accepted: Vec<CloneGroup> = Vec::new();
    for group in groups {
        if !is_subsumed_by(&group, &accepted) {
            accepted.push(group);
        }
    }

    accepted
}

fn build_suffix_array(text: &[usize]) -> Vec<usize> {
    let mut sa: Vec<usize> = (0..text.len()).collect();
    sa.sort_by(|&a, &b| text[a..].cmp(&text[b..]));
    sa
}

/// Count how many tokens match between `text[i+start..]` and `text[j+start..]`.
fn count_common_prefix(text: &[usize], i: usize, j: usize, start: usize) -> usize {
    let n = text.len();
    let mut len = 0;
    while i + start + len < n
        && j + start + len < n
        && text[i + start + len] == text[j + start + len]
    {
        len += 1;
    }
    len
}

fn build_lcp_array(text: &[usize], sa: &[usize]) -> Vec<usize> {
    let n = text.len();
    let mut rank = vec![0usize; n];
    for (i, &s) in sa.iter().enumerate() {
        rank[s] = i;
    }

    let mut lcp = vec![0usize; n];
    let mut h: usize = 0;

    for i in 0..n {
        if rank[i] == 0 {
            h = 0;
            continue;
        }
        let j = sa[rank[i] - 1];
        h += count_common_prefix(text, i, j, h);
        lcp[rank[i]] = h;
        h = h.saturating_sub(1);
    }

    lcp
}

/// Pop stack entries with depth > `cur`, recording valid intervals.
/// Returns the leftmost bound seen during popping.
fn pop_and_record(
    stack: &mut Vec<(usize, usize)>,
    intervals: &mut Vec<(usize, usize, usize)>,
    cur: usize,
    i: usize,
    min_tokens: usize,
) -> usize {
    let mut lb = i - 1;
    while stack.last().is_some_and(|&(d, _)| d > cur) {
        let (depth, left) = stack.pop().unwrap();
        lb = left;
        if depth >= min_tokens && i - 1 > left {
            intervals.push((depth, left, i - 1));
        }
    }
    lb
}

/// Enumerate all maximal LCP intervals with depth >= `min_tokens`.
/// Returns `(depth, left_bound, right_bound)` for each interval.
fn extract_lcp_intervals(
    sa: &[usize],
    lcp: &[usize],
    min_tokens: usize,
) -> Vec<(usize, usize, usize)> {
    let n = sa.len();
    let mut intervals = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new(); // (depth, left_bound)

    // Standard LCP interval traversal — `i` tracks position for boundary
    // arithmetic, not just array indexing. Rewriting as an iterator obscures
    // the algorithm.
    #[allow(clippy::needless_range_loop)]
    for i in 1..=n {
        let cur = lcp.get(i).copied().unwrap_or(0);
        let lb = pop_and_record(&mut stack, &mut intervals, cur, i, min_tokens);

        if should_push_interval(cur, min_tokens, &stack) {
            stack.push((cur, lb));
        }
    }

    intervals
}

fn should_push_interval(cur: usize, min_tokens: usize, stack: &[(usize, usize)]) -> bool {
    cur >= min_tokens && stack.last().is_none_or(|&(d, _)| cur > d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_similarity::{parse_source, rust};

    const LOW_TOKEN_THRESHOLD: usize = 5;
    const IMPOSSIBLY_HIGH_THRESHOLD: usize = 1000;

    fn parse_tokens(source: &str, path: &str) -> Vec<Token> {
        parse_source(source, path, std::path::Path::new(path), &rust::Rust)
            .unwrap()
            .tokens()
    }

    /// fn $ID ( $ID : $ID ) -> $ID { $ID + $LIT } = 14 tokens
    const CLONE_PAIR_TOKEN_COUNT: usize = 14;

    /// Two single-line functions with identical structure but different names/types.
    fn rust_clone_pair() -> [Vec<Token>; 2] {
        [
            parse_tokens("fn f(x: i32) -> i32 { x + 1 }", "a.rs"),
            parse_tokens("fn g(y: u32) -> u32 { y + 1 }", "b.rs"),
        ]
    }

    /// Detect clones in the standard clone pair with the given threshold.
    fn detect_pair(min_tokens: usize) -> Vec<CloneGroup> {
        let [a, b] = rust_clone_pair();
        detect_clones(&[a, b], min_tokens)
    }

    #[test]
    fn detects_within_file_duplication() {
        let source = "fn foo(x: i32) -> i32 { x + 1 }\nfn bar(y: i32) -> i32 { y + 1 }";
        let tokens = parse_tokens(source, "same.rs");

        let groups = detect_clones(&[tokens], LOW_TOKEN_THRESHOLD);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].occurrences.len(), 2);
        assert_eq!(groups[0].occurrences[0].source_idx, 0);
        assert_eq!(groups[0].occurrences[1].source_idx, 0);
    }

    #[test]
    fn detects_cross_file_duplication() {
        let a = parse_tokens("fn calc(x: f64, y: f64) -> f64 { x + y }", "a.rs");
        let b = parse_tokens("fn add(a: i32, b: i32) -> i32 { a + b }", "b.rs");

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].occurrences[0].source_idx, 0);
        assert_eq!(groups[0].occurrences[1].source_idx, 1);
    }

    #[test]
    fn groups_three_identical_regions_into_one_group() {
        let [a, b] = rust_clone_pair();
        let c = parse_tokens("fn h(z: f64) -> f64 { z + 1 }", "c.rs");

        let groups = detect_clones(&[a, b, c], LOW_TOKEN_THRESHOLD);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].occurrences.len(), 3);
    }

    #[test]
    fn no_clones_in_distinct_code() {
        let a = parse_tokens("let x = 1 + 2;", "a.rs");
        let b = parse_tokens("if true { return false; }", "b.rs");

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        assert!(groups.is_empty());
    }

    #[test]
    fn filters_matches_below_min_tokens() {
        assert!(detect_pair(IMPOSSIBLY_HIGH_THRESHOLD).is_empty());
    }

    #[test]
    fn reports_token_count_at_least_min_tokens() {
        let groups = detect_pair(LOW_TOKEN_THRESHOLD);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].token_count, CLONE_PAIR_TOKEN_COUNT);
    }

    #[test]
    fn occurrences_carry_token_positions() {
        let groups = detect_pair(LOW_TOKEN_THRESHOLD);

        assert_eq!(groups[0].occurrences[0].source_idx, 0);
        assert_eq!(groups[0].occurrences[0].token_start, 0);
        assert_eq!(groups[0].occurrences[1].source_idx, 1);
        assert_eq!(groups[0].occurrences[1].token_start, 0);
    }

    #[test]
    fn same_input_produces_identical_output() {
        assert_eq!(
            detect_pair(LOW_TOKEN_THRESHOLD),
            detect_pair(LOW_TOKEN_THRESHOLD),
        );
    }

    #[test]
    fn empty_source_produces_no_clones() {
        let a = parse_tokens("", "a.rs");
        let b = parse_tokens("fn f(x: i32) -> i32 { x + 1 }", "b.rs");

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        assert!(groups.is_empty());
    }

    #[test]
    fn min_tokens_zero_returns_empty() {
        assert!(detect_pair(0).is_empty());
    }

    #[test]
    fn single_source_without_duplication_produces_no_clones() {
        let a = parse_tokens("fn f(x: i32) -> i32 { x + 1 }", "a.rs");

        let groups = detect_clones(&[a], LOW_TOKEN_THRESHOLD);

        assert!(groups.is_empty());
    }

    #[test]
    fn comment_only_source_produces_no_clones() {
        let a = parse_tokens("// just a comment\n/* block comment */", "a.rs");
        let b = parse_tokens("// another comment\n/* block */", "b.rs");

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        assert!(groups.is_empty());
    }

    #[test]
    fn clone_at_exact_min_tokens_is_detected() {
        let groups = detect_pair(CLONE_PAIR_TOKEN_COUNT);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].token_count, CLONE_PAIR_TOKEN_COUNT);
    }

    #[test]
    fn clone_one_below_min_tokens_is_not_detected() {
        assert!(detect_pair(CLONE_PAIR_TOKEN_COUNT + 1).is_empty());
    }

    #[test]
    fn preserves_line_ranges_for_multi_line_clones() {
        let src_a = "\
fn process(
    x: i32,
    y: i32,
) -> i32 {
    x + y
}";
        let src_b = "\
fn compute(
    a: u64,
    b: u64,
) -> u64 {
    a + b
}";

        let tokens_a = parse_tokens(src_a, "a.rs");
        let tokens_b = parse_tokens(src_b, "b.rs");
        let sources = [tokens_a.clone(), tokens_b.clone()];

        let groups = detect_clones(&sources, LOW_TOKEN_THRESHOLD);

        assert_eq!(groups.len(), 1);
        let group = &groups[0];

        for occ in &group.occurrences {
            let tokens = &sources[occ.source_idx];
            let first = &tokens[occ.token_start];
            let last = &tokens[occ.token_start + group.token_count - 1];

            assert_eq!(first.start_line, 1, "clone should start at line 1");
            assert_eq!(last.end_line, 6, "clone should end at line 6");
        }
    }

    #[test]
    fn discards_groups_subsumed_by_a_longer_match() {
        let a = parse_tokens("fn f(x: i32, y: i32) -> i32 { x + y + 1 }", "a.rs");
        let b = parse_tokens("fn g(a: u32, b: u32) -> u32 { a + b + 1 }", "b.rs");

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        // The suffix array finds many overlapping sub-sequences, but only
        // the longest match should survive — shorter ones are fully contained.
        assert_eq!(groups.len(), 1);
    }
}
