use super::SourceTokens;
use std::collections::HashMap;

/// A group of code regions that share the same normalized token sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct CloneGroup {
    pub token_count: usize,
    pub occurrences: Vec<Occurrence>,
}

/// A single occurrence of a clone within a source file.
#[derive(Debug, Clone, PartialEq)]
pub struct Occurrence {
    pub source_id: String,
    pub start_line: usize,
    pub end_line: usize,
}

struct TokenSequence {
    concat: Vec<usize>,
    /// Maps each position in `concat` to `(file_index, token_index)`.
    /// `None` for sentinel positions.
    pos_map: Vec<Option<(usize, usize)>>,
}

/// Detect clone groups across the given source token sequences.
///
/// Builds a generalized suffix array over the concatenated (normalized) token
/// streams, then walks the LCP array to find all maximal repeated regions
/// of at least `min_tokens` length.
#[must_use]
pub fn detect_clones(sources: &[SourceTokens], min_tokens: usize) -> Vec<CloneGroup> {
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
/// mapping each position back to its source file and token index.
fn build_token_sequence(sources: &[SourceTokens]) -> TokenSequence {
    // Real token IDs start at 0. Sentinels use the high end of usize
    // (usize::MAX, usize::MAX-1, …) so they never collide with real IDs.
    let mut vocab: HashMap<&str, usize> = HashMap::new();
    let mut concat: Vec<usize> = Vec::new();
    let mut pos_map: Vec<Option<(usize, usize)>> = Vec::new();

    for (file_idx, source) in sources.iter().enumerate() {
        for (tok_idx, token) in source.tokens.iter().enumerate() {
            let next_id = vocab.len();
            let id = *vocab.entry(token.text.as_str()).or_insert(next_id);
            concat.push(id);
            pos_map.push(Some((file_idx, tok_idx)));
        }
        concat.push(usize::MAX - file_idx); // unique sentinel per file
        pos_map.push(None);
    }

    TokenSequence { concat, pos_map }
}

/// Convert LCP intervals into clone groups by resolving positions back to
/// source files and line numbers.
fn intervals_to_groups(
    intervals: &[(usize, usize, usize)],
    sa: &[usize],
    pos_map: &[Option<(usize, usize)>],
    sources: &[SourceTokens],
) -> Vec<CloneGroup> {
    let mut groups: Vec<CloneGroup> = Vec::new();

    for &(depth, left, right) in intervals {
        let mut occurrences: Vec<Occurrence> = Vec::new();

        for &pos in &sa[left..=right] {
            let Some((file_idx, tok_idx)) = pos_map[pos] else {
                continue; // sentinel
            };

            let source = &sources[file_idx];
            let end_tok = (tok_idx + depth - 1).min(source.tokens.len() - 1);
            occurrences.push(Occurrence {
                source_id: source.source_id.clone(),
                start_line: source.tokens[tok_idx].start_line,
                end_line: source.tokens[end_tok].end_line,
            });
        }

        occurrences.sort_by(|a, b| {
            a.source_id
                .cmp(&b.source_id)
                .then(a.start_line.cmp(&b.start_line))
        });
        occurrences.dedup();

        if occurrences.len() >= 2 {
            groups.push(CloneGroup {
                token_count: depth,
                occurrences,
            });
        }
    }

    groups
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
    'outer: for group in groups {
        for prev in &accepted {
            let all_contained = group.occurrences.iter().all(|occ| {
                prev.occurrences.iter().any(|p| {
                    p.source_id == occ.source_id
                        && p.start_line <= occ.start_line
                        && p.end_line >= occ.end_line
                })
            });
            if all_contained {
                continue 'outer;
            }
        }
        accepted.push(group);
    }

    accepted
}

fn build_suffix_array(text: &[usize]) -> Vec<usize> {
    let mut sa: Vec<usize> = (0..text.len()).collect();
    sa.sort_by(|&a, &b| text[a..].cmp(&text[b..]));
    sa
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
        while i + h < n && j + h < n && text[i + h] == text[j + h] {
            h += 1;
        }
        lcp[rank[i]] = h;
        h = h.saturating_sub(1);
    }

    lcp
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
        let cur = if i < n { lcp[i] } else { 0 };
        let mut lb = i - 1;

        while let Some(&(d, _)) = stack.last() {
            if cur >= d {
                break;
            }
            let (depth, left) = stack.pop().unwrap();
            lb = left;

            if depth >= min_tokens && i - 1 > left {
                intervals.push((depth, left, i - 1));
            }
        }

        if cur >= min_tokens && (stack.is_empty() || cur > stack.last().unwrap().0) {
            stack.push((cur, lb));
        }
    }

    intervals
}
