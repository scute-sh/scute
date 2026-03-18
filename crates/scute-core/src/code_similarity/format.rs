use super::detect::{CloneGroup, Occurrence};
use super::evaluate::{SourceContext, occurrence_tokens};
use crate::{Evaluation, Evidence, Thresholds};

pub fn format_evaluation(
    group: &CloneGroup,
    thresholds: &Thresholds,
    sources: &[SourceContext],
) -> Evaluation {
    let evidence = group
        .occurrences
        .iter()
        .map(|occ| format_evidence(occ, group.token_count, sources))
        .collect();

    let observed = u64::try_from(group.token_count).unwrap_or(u64::MAX);
    let target = format_target(&group.occurrences[0], sources);

    Evaluation::completed(target, observed, thresholds.clone(), evidence)
}

fn format_target(occ: &Occurrence, sources: &[SourceContext]) -> String {
    let src = &sources[occ.source_idx];
    let start_line = src.tokens.get(occ.token_start).map_or(0, |t| t.start_line);
    format!("{}:{start_line}", src.tree.source_id())
}

fn format_evidence(occ: &Occurrence, token_count: usize, sources: &[SourceContext]) -> Evidence {
    let source = &sources[occ.source_idx];
    let matched_tokens = occurrence_tokens(occ, token_count, sources);

    let start_line = matched_tokens.first().map_or(0, |t| t.start_line);
    let end_line = matched_tokens.last().map_or(0, |t| t.end_line);

    let found = match representative_line(source.content, start_line, end_line) {
        Some(line) => format!("{token_count} duplicated tokens, e.g. `{line}`"),
        None => format!("{token_count} duplicated tokens"),
    };

    Evidence {
        rule: None,
        location: Some(format!(
            "{}:{start_line}-{end_line}",
            source.tree.source_id()
        )),
        found,
        expected: None,
    }
}

/// Pick the first non-trivial line from the given range as a representative snippet.
fn representative_line(content: &str, start_line: usize, end_line: usize) -> Option<&str> {
    content
        .lines()
        .skip(start_line.saturating_sub(1))
        .take(end_line.saturating_sub(start_line) + 1)
        .map(str::trim)
        .find(|line| !is_trivial_line(line))
}

/// A line is "trivial" if it's only punctuation and whitespace (closing braces,
/// semicolons, etc.). We skip these when picking a representative snippet.
fn is_trivial_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.chars().all(|c| c.is_ascii_punctuation())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Outcome;
    use crate::code_similarity::tree::SourceTreeBuilder;

    #[test]
    fn empty_string_is_trivial() {
        assert!(is_trivial_line(""));
    }

    #[test]
    fn closing_brace_is_trivial() {
        assert!(is_trivial_line("}"));
    }

    #[test]
    fn closing_brace_semicolon_is_trivial() {
        assert!(is_trivial_line("};"));
    }

    #[test]
    fn whitespace_only_is_trivial() {
        assert!(is_trivial_line("   \t  "));
    }

    #[test]
    fn fn_declaration_is_not_trivial() {
        assert!(!is_trivial_line("fn foo()"));
    }

    /// Format a single-source clone and return (observed, evidence).
    fn format_single_source(
        content: &str,
        token_specs: &[(&str, usize, usize)],
    ) -> (u64, Vec<Evidence>) {
        let mut b = SourceTreeBuilder::new("a.rs".to_string());
        for &(text, start, end) in token_specs {
            b.add_token(text.to_string(), start, end);
        }
        let tree = b.build();
        let tokens = tree.tokens();
        let token_count = tokens.len();
        let sources = [SourceContext {
            tree: &tree,
            tokens: &tokens,
            content,
        }];
        let group = CloneGroup {
            token_count,
            occurrences: vec![
                Occurrence {
                    source_idx: 0,
                    token_start: 0,
                },
                Occurrence {
                    source_idx: 0,
                    token_start: 0,
                },
            ],
        };
        let eval = format_evaluation(
            &group,
            &Thresholds {
                warn: None,
                fail: None,
            },
            &sources,
        );
        let Outcome::Completed {
            observed, evidence, ..
        } = eval.outcome
        else {
            panic!("expected completed evaluation");
        };
        (observed, evidence)
    }

    #[test]
    fn format_evaluation_includes_all_occurrence_locations() {
        let mut b_a = SourceTreeBuilder::new("a.rs".to_string());
        b_a.add_token("fn".to_string(), 1, 1);
        b_a.add_token("$ID".to_string(), 1, 1);
        let tree_a = b_a.build();
        let tokens_a = tree_a.tokens();

        let mut b_b = SourceTreeBuilder::new("b.rs".to_string());
        b_b.add_token("fn".to_string(), 3, 3);
        b_b.add_token("$ID".to_string(), 3, 3);
        let tree_b = b_b.build();
        let tokens_b = tree_b.tokens();

        let sources = [
            SourceContext {
                tree: &tree_a,
                tokens: &tokens_a,
                content: "fn foo()",
            },
            SourceContext {
                tree: &tree_b,
                tokens: &tokens_b,
                content: "stuff\nmore\nfn bar()",
            },
        ];
        let group = CloneGroup {
            token_count: 2,
            occurrences: vec![
                Occurrence {
                    source_idx: 0,
                    token_start: 0,
                },
                Occurrence {
                    source_idx: 1,
                    token_start: 0,
                },
            ],
        };

        let eval = format_evaluation(
            &group,
            &Thresholds {
                warn: Some(0),
                fail: Some(0),
            },
            &sources,
        );

        let Outcome::Completed { evidence, .. } = &eval.outcome else {
            panic!("expected completed evaluation");
        };
        assert_eq!(evidence.len(), 2);
        assert!(evidence[0].location.as_ref().unwrap().contains("a.rs"));
        assert!(evidence[1].location.as_ref().unwrap().contains("b.rs"));
    }

    #[test]
    fn format_evaluation_picks_non_trivial_snippet() {
        let (_, evidence) = format_single_source("}\nfn foo()", &[("}", 1, 1), ("fn", 2, 2)]);

        assert!(
            evidence[0].found.contains("fn foo()"),
            "should pick non-trivial line, got: {}",
            evidence[0].found,
        );
    }

    #[test]
    fn format_evaluation_reports_token_count_as_observed() {
        let (observed, _) = format_single_source(
            "fn foo() {",
            &[
                ("fn", 1, 1),
                ("$ID", 1, 1),
                ("(", 1, 1),
                (")", 1, 1),
                ("{", 1, 1),
            ],
        );

        assert_eq!(observed, 5);
    }
}
