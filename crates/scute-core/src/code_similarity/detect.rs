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

#[derive(Clone, Copy)]
struct TokenPosition {
    source_idx: usize,
    token_idx: usize,
}

struct LcpInterval {
    depth: usize,
    left: usize,
    right: usize,
}

struct StackEntry {
    depth: usize,
    left_bound: usize,
}

struct TokenSequence {
    concat: Vec<usize>,
    pos_map: Vec<Option<TokenPosition>>,
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

    let sequence = build_token_sequence(sources);
    if sequence.concat.len() < 2 {
        return vec![];
    }

    let suffix_array = build_suffix_array(&sequence.concat);
    let lcp = build_lcp_array(&sequence.concat, &suffix_array);
    let intervals = extract_lcp_intervals(&suffix_array, &lcp, min_tokens);

    let groups = intervals_to_groups(&intervals, &suffix_array, &sequence.pos_map, sources);
    filter_maximal_groups(groups)
}

/// Concatenate all token streams with unique sentinels between them,
/// mapping each position back to its source and token index.
fn build_token_sequence(sources: &[Vec<Token>]) -> TokenSequence {
    // Real token IDs start at 0. Sentinels use the high end of usize
    // (usize::MAX, usize::MAX-1, …) so they never collide with real IDs.
    let mut vocab: HashMap<&str, usize> = HashMap::new();
    let mut concat: Vec<usize> = Vec::new();
    let mut pos_map: Vec<Option<TokenPosition>> = Vec::new();

    for (source_idx, tokens) in sources.iter().enumerate() {
        for (token_idx, tok) in tokens.iter().enumerate() {
            let next_id = vocab.len();
            let id = *vocab.entry(tok.text.as_str()).or_insert(next_id);
            concat.push(id);
            pos_map.push(Some(TokenPosition {
                source_idx,
                token_idx,
            }));
        }
        concat.push(usize::MAX - source_idx); // unique sentinel per source
        pos_map.push(None);
    }

    TokenSequence { concat, pos_map }
}

fn intervals_to_groups(
    intervals: &[LcpInterval],
    suffix_array: &[usize],
    pos_map: &[Option<TokenPosition>],
    sources: &[Vec<Token>],
) -> Vec<CloneGroup> {
    intervals
        .iter()
        .filter_map(|interval| {
            let occurrences = collect_occurrences(
                &suffix_array[interval.left..=interval.right],
                pos_map,
                sources,
                interval.depth,
            );
            (occurrences.len() >= 2).then_some(CloneGroup {
                token_count: interval.depth,
                occurrences,
            })
        })
        .collect()
}

fn collect_occurrences(
    sa_slice: &[usize],
    pos_map: &[Option<TokenPosition>],
    sources: &[Vec<Token>],
    depth: usize,
) -> Vec<Occurrence> {
    let mut occurrences: Vec<Occurrence> = sa_slice
        .iter()
        .filter_map(|&pos| {
            let tp = pos_map[pos]?;
            (tp.token_idx + depth <= sources[tp.source_idx].len()).then_some(Occurrence {
                source_idx: tp.source_idx,
                token_start: tp.token_idx,
            })
        })
        .collect();

    occurrences.sort_by(|a, b| {
        a.source_idx
            .cmp(&b.source_idx)
            .then(a.token_start.cmp(&b.token_start))
    });
    occurrences.dedup();
    occurrences.dedup_by(|next, prev| {
        prev.source_idx == next.source_idx && next.token_start < prev.token_start + depth
    });
    occurrences
}

/// Check if every occurrence in `candidate` overlaps with
/// some occurrence of an already-accepted group.
fn is_subsumed_by(candidate: &CloneGroup, accepted: &[CloneGroup]) -> bool {
    accepted.iter().any(|prev| {
        candidate.occurrences.iter().all(|occ| {
            let occ_end = occ.token_start + candidate.token_count;
            prev.occurrences.iter().any(|p| {
                let p_end = p.token_start + prev.token_count;
                p.source_idx == occ.source_idx && occ.token_start < p_end && p.token_start < occ_end
            })
        })
    })
}

/// Keep only maximal matches: discard groups where every occurrence
/// overlaps with an already-accepted longer group.
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

fn build_lcp_array(text: &[usize], suffix_array: &[usize]) -> Vec<usize> {
    let n = text.len();
    let mut rank = vec![0usize; n];
    for (i, &s) in suffix_array.iter().enumerate() {
        rank[s] = i;
    }

    let mut lcp = vec![0usize; n];
    let mut h: usize = 0;

    for i in 0..n {
        if rank[i] == 0 {
            h = 0;
            continue;
        }
        let j = suffix_array[rank[i] - 1];
        h += count_common_prefix(text, i, j, h);
        lcp[rank[i]] = h;
        h = h.saturating_sub(1);
    }

    lcp
}

/// Collapse stack entries deeper than `current_depth`, recording completed intervals.
/// Returns the leftmost bound from collapsed entries.
fn collapse_deeper(
    stack: &mut Vec<StackEntry>,
    intervals: &mut Vec<LcpInterval>,
    current_depth: usize,
    right_bound: usize,
    min_tokens: usize,
) -> usize {
    let mut left_bound = right_bound;
    while stack
        .last()
        .is_some_and(|entry| entry.depth > current_depth)
    {
        let entry = stack.pop().unwrap();
        left_bound = entry.left_bound;
        if entry.depth >= min_tokens && right_bound > entry.left_bound {
            intervals.push(LcpInterval {
                depth: entry.depth,
                left: entry.left_bound,
                right: right_bound,
            });
        }
    }
    left_bound
}

fn extract_lcp_intervals(
    suffix_array: &[usize],
    lcp: &[usize],
    min_tokens: usize,
) -> Vec<LcpInterval> {
    let n = suffix_array.len();
    let mut intervals = Vec::new();
    let mut stack: Vec<StackEntry> = Vec::new();

    #[allow(clippy::needless_range_loop)] // i used for boundary arithmetic
    for i in 1..=n {
        let current_depth = lcp.get(i).copied().unwrap_or(0);
        let right_bound = i - 1;
        let left_bound = collapse_deeper(
            &mut stack,
            &mut intervals,
            current_depth,
            right_bound,
            min_tokens,
        );

        if current_depth >= min_tokens
            && stack.last().is_none_or(|entry| current_depth > entry.depth)
        {
            stack.push(StackEntry {
                depth: current_depth,
                left_bound,
            });
        }
    }

    intervals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_similarity::check::languages;
    use crate::code_similarity::parse_source;
    use crate::files::SourceFile;

    const LOW_TOKEN_THRESHOLD: usize = 5;
    const IMPOSSIBLY_HIGH_THRESHOLD: usize = 1000;

    fn parse_tokens(source: &str, path: &str) -> Vec<Token> {
        let file = SourceFile {
            path: path.into(),
            content: source.into(),
        };
        parse_source(&file, &languages()).unwrap().tokens()
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
    fn drops_clone_groups_where_all_occurrences_overlap() {
        let source = r"
            const ITEMS = [
                'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j',
                'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't',
            ];
        ";
        let tokens = parse_tokens(source, "list.ts");
        let above_half_the_list = 21; // 20 items = 40 tokens; any match above half must overlap

        let groups = detect_clones(&[tokens], above_half_the_list);

        assert!(groups.is_empty(), "got {groups:#?}");
    }

    #[test]
    fn subsumes_shorter_groups_that_overlap_with_longer_accepted_group() {
        let source = r"
            const ITEMS = new Set([
                'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j',
                'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't',
                'u', 'v', 'w', 'x', 'y', 'z', 'aa', 'bb', 'cc', 'dd',
            ]);
        ";
        let tokens = parse_tokens(source, "list.ts");

        let groups = detect_clones(&[tokens], LOW_TOKEN_THRESHOLD);

        assert!(
            groups.len() <= 1,
            "expected at most 1 group, got {}",
            groups.len()
        );
    }

    #[test]
    fn preserves_independent_groups_with_no_overlapping_occurrences() {
        let a = parse_tokens("fn f(x: i32) -> i32 { x + 1 }", "a.rs");
        let b = parse_tokens("fn g(y: u32) -> u32 { y + 1 }", "b.rs");
        let c = parse_tokens(
            "struct Foo { a: i32, b: i32, c: i32, d: i32, e: i32 }",
            "c.rs",
        );
        let d = parse_tokens(
            "struct Bar { a: u64, b: u64, c: u64, d: u64, e: u64 }",
            "d.rs",
        );

        let groups = detect_clones(&[a, b, c, d], LOW_TOKEN_THRESHOLD);

        assert_eq!(groups.len(), 2, "got {groups:#?}");
    }

    #[test]
    fn preserves_group_when_some_occurrences_do_not_overlap_with_accepted() {
        let a = parse_tokens(
            "fn f(x: i32, y: i32) -> i32 { if x > 0 { return x; } else { return 0; } }",
            "a.rs",
        );
        let b = parse_tokens(
            "fn g(a: u32, b: u32) -> u32 { if a > 0 { return a; } else { return 0; } }",
            "b.rs",
        );
        let c = parse_tokens(
            "fn h(z: f64) -> f64 { if z > 0 { return z; } else { return 0; } }",
            "c.rs",
        );

        let groups = detect_clones(&[a, b, c], LOW_TOKEN_THRESHOLD);

        let has_three_way_group = groups.iter().any(|g| g.occurrences.len() == 3);
        assert!(
            has_three_way_group,
            "expected a group with 3 occurrences (a, b, c sharing the tail), got {groups:#?}"
        );
    }

    #[test]
    fn keeps_group_when_non_overlapping_occurrences_remain_after_merge() {
        let source_a = r"
            const ITEMS = [
                'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j',
                'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't',
            ];
        ";
        let source_b = r"
            const OTHER = [
                'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j',
                'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't',
            ];
        ";
        let tokens_a = parse_tokens(source_a, "a.ts");
        let tokens_b = parse_tokens(source_b, "b.ts");
        let above_half_the_list = 21;

        let groups = detect_clones(&[tokens_a, tokens_b], above_half_the_list);

        assert_eq!(groups.len(), 1, "got {groups:#?}");
        assert_eq!(groups[0].occurrences.len(), 2);
        assert_eq!(groups[0].occurrences[0].source_idx, 0);
        assert_eq!(groups[0].occurrences[1].source_idx, 1);
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
        // the longest match should survive. Shorter ones are fully contained.
        assert_eq!(groups.len(), 1);
    }
}
