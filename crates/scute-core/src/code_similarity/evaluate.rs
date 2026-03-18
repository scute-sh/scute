use super::detect::{CloneGroup, Occurrence};
use super::tree::{SourceTree, Token};
use crate::{Evaluation, Thresholds};

/// One source file's parsed tree, tokens, and raw content, bundled together.
///
/// Replaces the parallel-array pattern where `trees[i]`, `tokens[i]`, and
/// `contents[i]` were passed separately and indexed by `source_idx`.
pub(super) struct SourceContext<'a> {
    pub tree: &'a SourceTree,
    pub tokens: &'a [Token],
    pub content: &'a str,
}

/// Evaluate clone groups using structural context from the trees.
///
/// For each group, walks up from matched tokens to determine context:
/// - Same-contract groups (all occurrences inside the same trait impl) are excluded
/// - Test-only groups get separate thresholds
/// - Everything else gets production thresholds
pub fn evaluate_groups(
    groups: &[&CloneGroup],
    sources: &[SourceContext],
    thresholds: &Thresholds,
    test_thresholds: &Thresholds,
) -> Vec<Evaluation> {
    groups
        .iter()
        .filter_map(|group| {
            if is_same_contract_group(group, sources) {
                return None;
            }
            let effective = if is_test_only_group(group, sources) {
                test_thresholds
            } else {
                thresholds
            };
            Some(super::format::format_evaluation(group, effective, sources))
        })
        .collect()
}

/// Returns the tokens for the exact range in this occurrence.
pub fn occurrence_tokens<'a>(
    occ: &Occurrence,
    token_count: usize,
    sources: &'a [SourceContext],
) -> &'a [Token] {
    let tokens = sources[occ.source_idx].tokens;
    let end = occ.token_start + token_count;
    debug_assert!(
        end <= tokens.len(),
        "occurrence range {start}..{end} exceeds token count {len}",
        start = occ.token_start,
        len = tokens.len(),
    );
    &tokens[occ.token_start..end.min(tokens.len())]
}

/// Returns `true` if every occurrence's tokens all live inside an
/// implementation of the same contract.
fn is_same_contract_group(group: &CloneGroup, sources: &[SourceContext]) -> bool {
    let mut contract: Option<&str> = None;
    for occ in &group.occurrences {
        let src = &sources[occ.source_idx];
        let matched_tokens = occurrence_tokens(occ, group.token_count, sources);
        let Some(name) = all_same_contract(src.tree, matched_tokens) else {
            return false;
        };
        match contract {
            None => contract = Some(name),
            Some(existing) if existing == name => {}
            Some(_) => return false,
        }
    }
    contract.is_some()
}

/// If all tokens are inside the same contract, return its name.
fn all_same_contract<'a>(tree: &'a SourceTree, tokens: &[Token]) -> Option<&'a str> {
    let mut name: Option<&str> = None;
    for tok in tokens {
        let contract = tree.enclosing_contract(tok.node_index)?;
        match name {
            None => name = Some(contract),
            Some(existing) if existing == contract => {}
            Some(_) => return None,
        }
    }
    name
}

fn is_test_only_group(group: &CloneGroup, sources: &[SourceContext]) -> bool {
    group.occurrences.iter().all(|occ| {
        let src = &sources[occ.source_idx];
        let matched_tokens = occurrence_tokens(occ, group.token_count, sources);
        matched_tokens
            .iter()
            .all(|tok| src.tree.is_in_test_region(tok.node_index))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_similarity::tree::{NodeKind, SourceTreeBuilder};

    const CLONE_TOKENS: &[&str] = &["fn", "$ID", "(", ")", "{", "}"];

    // Strict production thresholds: 6 tokens (CLONE_TOKENS) will fail.
    const STRICT: Thresholds = Thresholds {
        warn: Some(3),
        fail: Some(5),
    };
    // Lenient test thresholds: only very large clones warn/fail.
    const LENIENT: Thresholds = Thresholds {
        warn: Some(100),
        fail: Some(200),
    };

    fn build_tree(path: &str, container: Option<NodeKind>, token_texts: &[&str]) -> SourceTree {
        let mut b = SourceTreeBuilder::new(path.to_string());
        let has_container = container.is_some();
        if let Some(kind) = container {
            b.open_container(kind);
        }
        for (i, text) in token_texts.iter().enumerate() {
            b.add_token(text.to_string(), i + 1, i + 1);
        }
        if has_container {
            b.close_container();
        }
        b.build()
    }

    #[allow(clippy::unnecessary_wraps)]
    fn contract(name: &str) -> Option<NodeKind> {
        Some(NodeKind::Contract {
            name: name.to_string(),
        })
    }

    #[allow(clippy::unnecessary_wraps)]
    fn test_region() -> Option<NodeKind> {
        Some(NodeKind::TestRegion)
    }

    /// Build contexts from trees and run evaluation with standard thresholds.
    fn run_evaluation(trees: &[SourceTree], contents: &[&str]) -> Vec<Evaluation> {
        let all_tokens: Vec<Vec<Token>> = trees.iter().map(SourceTree::tokens).collect();
        let sources: Vec<SourceContext> = trees
            .iter()
            .zip(&all_tokens)
            .zip(contents)
            .map(|((tree, tokens), &content)| SourceContext {
                tree,
                tokens,
                content,
            })
            .collect();
        let group = CloneGroup {
            token_count: CLONE_TOKENS.len(),
            occurrences: (0..trees.len())
                .map(|idx| Occurrence {
                    source_idx: idx,
                    token_start: 0,
                })
                .collect(),
        };
        evaluate_groups(&[&group], &sources, &STRICT, &LENIENT)
    }

    #[test]
    fn excludes_same_contract_groups() {
        let trees = [
            build_tree("a.rs", contract("Render"), CLONE_TOKENS),
            build_tree("b.rs", contract("Render"), CLONE_TOKENS),
        ];

        assert!(run_evaluation(&trees, &["", ""]).is_empty());
    }

    #[test]
    fn reports_groups_across_different_contracts() {
        let trees = [
            build_tree("a.rs", contract("Render"), CLONE_TOKENS),
            build_tree("b.rs", contract("Format"), CLONE_TOKENS),
        ];

        assert_eq!(run_evaluation(&trees, &["", ""]).len(), 1);
    }

    #[test]
    fn reports_mixed_contract_and_free_code() {
        let trees = [
            build_tree("a.rs", contract("Render"), CLONE_TOKENS),
            build_tree("b.rs", None, CLONE_TOKENS),
        ];

        assert_eq!(run_evaluation(&trees, &["", ""]).len(), 1);
    }

    #[test]
    fn applies_test_thresholds_for_test_only_groups() {
        let trees = [
            build_tree("a.rs", test_region(), CLONE_TOKENS),
            build_tree("b.rs", test_region(), CLONE_TOKENS),
        ];

        // 6 tokens with STRICT (fail at 5) would fail.
        // But test-only groups use LENIENT (fail at 200), so it passes.
        let result = run_evaluation(&trees, &["fn f() {}", "fn g() {}"]);
        assert_eq!(result.len(), 1);
        assert!(
            result[0].is_pass(),
            "test-only group should use lenient thresholds"
        );
    }

    #[test]
    fn applies_production_thresholds_for_mixed_groups() {
        let trees = [
            build_tree("a.rs", test_region(), CLONE_TOKENS),
            build_tree("b.rs", None, CLONE_TOKENS),
        ];

        // Mixed group uses STRICT (fail at 5). 6 tokens → fail.
        let result = run_evaluation(&trees, &["fn f() {}", "fn g() {}"]);
        assert_eq!(result.len(), 1);
        assert!(
            result[0].is_fail(),
            "mixed group should use production thresholds"
        );
    }

    #[test]
    fn occurrence_tokens_returns_exact_slice() {
        let tree = build_tree("a.rs", None, &["fn", "$ID", "(", ")", "{", "return", "}"]);
        let tokens = tree.tokens();
        let sources = [SourceContext {
            tree: &tree,
            tokens: &tokens,
            content: "",
        }];

        let occ = Occurrence {
            source_idx: 0,
            token_start: 2,
        };
        let slice = occurrence_tokens(&occ, 3, &sources);

        let texts: Vec<&str> = slice.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["(", ")", "{"]);
    }
}
