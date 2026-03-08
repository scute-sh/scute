mod check;
mod detect;
pub mod language;
mod parser;
mod tokenize;

pub use check::{CHECK_NAME, Definition, check};
pub use detect::{CloneGroup, Occurrence, detect_clones};
pub use language::{LanguageConfig, NodeRole};
pub use parser::{AstParser, ParseError, TreeSitterParser};
pub use tokenize::{Token, TokenizeError, tokenize};

/// A source entry for clone detection: raw source code + metadata.
pub struct SourceEntry<'a> {
    pub source: &'a str,
    pub source_id: &'a str,
    pub language: &'a LanguageConfig,
}

impl<'a> SourceEntry<'a> {
    #[must_use]
    pub fn new(source: &'a str, source_id: &'a str, language: &'a LanguageConfig) -> Self {
        Self {
            source,
            source_id,
            language,
        }
    }
}

/// Tokens from a single source file, ready for clone detection.
#[derive(Debug, Clone)]
pub struct SourceTokens {
    pub source_id: String,
    pub tokens: Vec<Token>,
}

impl SourceTokens {
    #[must_use]
    pub fn new(source_id: String, tokens: Vec<Token>) -> Self {
        Self { source_id, tokens }
    }
}

/// Detect clones in a set of source files.
///
/// Tokenizes each source entry, then runs clone detection over the
/// normalized token sequences. This is the main entry point for the
/// code similarity engine.
///
/// # Errors
///
/// Returns `TokenizeError` if any source entry fails to parse.
pub fn find_clones(
    entries: &[SourceEntry<'_>],
    min_tokens: usize,
) -> Result<Vec<CloneGroup>, TokenizeError> {
    let mut parser = TreeSitterParser::new();
    let sources: Vec<SourceTokens> = entries
        .iter()
        .map(|entry| {
            let tokens = tokenize(&mut parser, entry.source, entry.language)?;
            Ok(SourceTokens::new(entry.source_id.to_string(), tokens))
        })
        .collect::<Result<_, TokenizeError>>()?;

    Ok(detect_clones(&sources, min_tokens))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOW_TOKEN_THRESHOLD: usize = 5;
    const IMPOSSIBLY_HIGH_THRESHOLD: usize = 1000;

    fn tokenize_rust(source: &str, source_id: &str) -> SourceTokens {
        let mut parser = TreeSitterParser::new();
        let tokens = tokenize(&mut parser, source, &language::rust()).unwrap();
        SourceTokens::new(source_id.to_string(), tokens)
    }

    /// Two single-line functions with identical structure but different names/types.
    /// Produces 14 normalized tokens each: fn $ID ( $ID : $ID ) -> $ID { $ID + $LIT }
    fn rust_clone_pair() -> [SourceTokens; 2] {
        [
            tokenize_rust("fn f(x: i32) -> i32 { x + 1 }", "a.rs"),
            tokenize_rust("fn g(y: u32) -> u32 { y + 1 }", "b.rs"),
        ]
    }

    #[test]
    fn detects_within_file_duplication() {
        let source = "fn foo(x: i32) -> i32 { x + 1 }\nfn bar(y: i32) -> i32 { y + 1 }";

        let a = tokenize_rust(source, "same.rs");
        let groups = detect_clones(&[a], LOW_TOKEN_THRESHOLD);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].occurrences.len(), 2);
        assert_eq!(groups[0].occurrences[0].source_id, "same.rs");
        assert_eq!(groups[0].occurrences[1].source_id, "same.rs");
    }

    #[test]
    fn detects_cross_file_duplication() {
        let a = tokenize_rust("fn calc(x: f64, y: f64) -> f64 { x + y }", "a.rs");
        let b = tokenize_rust("fn add(a: i32, b: i32) -> i32 { a + b }", "b.rs");

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].occurrences[0].source_id, "a.rs");
        assert_eq!(groups[0].occurrences[1].source_id, "b.rs");
    }

    #[test]
    fn groups_three_identical_regions_into_one_group() {
        let [a, b] = rust_clone_pair();
        let c = tokenize_rust("fn h(z: f64) -> f64 { z + 1 }", "c.rs");

        let groups = detect_clones(&[a, b, c], LOW_TOKEN_THRESHOLD);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].occurrences.len(), 3);
    }

    #[test]
    fn no_clones_in_distinct_code() {
        let a = tokenize_rust("let x = 1 + 2;", "a.rs");
        let b = tokenize_rust("if true { return false; }", "b.rs");

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        assert!(groups.is_empty());
    }

    #[test]
    fn filters_matches_below_min_tokens() {
        let [a, b] = rust_clone_pair();

        let groups = detect_clones(&[a, b], IMPOSSIBLY_HIGH_THRESHOLD);

        assert!(groups.is_empty());
    }

    #[test]
    fn reports_token_count_at_least_min_tokens() {
        let [a, b] = rust_clone_pair();

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].token_count, 14);
    }

    #[test]
    fn occurrence_lines_are_coherent() {
        let [a, b] = rust_clone_pair();

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        assert_eq!(groups[0].occurrences[0].start_line, 1);
        assert_eq!(groups[0].occurrences[0].end_line, 1);
        assert_eq!(groups[0].occurrences[1].start_line, 1);
        assert_eq!(groups[0].occurrences[1].end_line, 1);
    }

    #[test]
    fn same_input_produces_identical_output() {
        let run = || {
            let [a, b] = rust_clone_pair();
            detect_clones(&[a, b], LOW_TOKEN_THRESHOLD)
        };

        assert_eq!(run(), run());
    }

    #[test]
    fn empty_source_produces_no_clones() {
        let a = tokenize_rust("", "a.rs");
        let b = tokenize_rust("fn f(x: i32) -> i32 { x + 1 }", "b.rs");

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        assert!(groups.is_empty());
    }

    #[test]
    fn min_tokens_zero_returns_empty() {
        let [a, b] = rust_clone_pair();

        let groups = detect_clones(&[a, b], 0);

        assert!(groups.is_empty());
    }

    #[test]
    fn syntax_errors_do_not_panic() {
        let mut parser = TreeSitterParser::new();
        let broken = tokenize(&mut parser, "fn f(x: i32 -> { x + }", &language::rust());

        assert!(broken.is_ok()); // tree-sitter recovers, never errors
    }

    #[test]
    fn single_source_without_duplication_produces_no_clones() {
        let a = tokenize_rust("fn f(x: i32) -> i32 { x + 1 }", "a.rs");

        let groups = detect_clones(&[a], LOW_TOKEN_THRESHOLD);

        assert!(groups.is_empty());
    }

    #[test]
    fn comment_only_source_produces_no_clones() {
        let a = tokenize_rust("// just a comment\n/* block comment */", "a.rs");
        let b = tokenize_rust("// another comment\n/* block */", "b.rs");

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        assert!(groups.is_empty());
    }

    #[test]
    fn clone_at_exact_min_tokens_is_detected() {
        let [a, b] = rust_clone_pair();

        let groups = detect_clones(&[a, b], 14);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].token_count, 14);
    }

    #[test]
    fn clone_one_below_min_tokens_is_not_detected() {
        let [a, b] = rust_clone_pair();

        let groups = detect_clones(&[a, b], 15);

        assert!(groups.is_empty());
    }

    #[test]
    fn multi_line_clone_tracks_correct_line_range() {
        let source_a = "\
fn f(x: i32) -> i32 {
    let result = x + 1;
    result * 2
}";
        let source_b = "\
fn g(y: u32) -> u32 {
    let result = y + 1;
    result * 2
}";
        let a = tokenize_rust(source_a, "a.rs");
        let b = tokenize_rust(source_b, "b.rs");

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].occurrences[0].start_line, 1);
        assert_eq!(groups[0].occurrences[0].end_line, 4);
    }

    #[test]
    fn discards_groups_subsumed_by_a_longer_match() {
        let a = tokenize_rust("fn f(x: i32, y: i32) -> i32 { x + y + 1 }", "a.rs");
        let b = tokenize_rust("fn g(a: u32, b: u32) -> u32 { a + b + 1 }", "b.rs");

        let groups = detect_clones(&[a, b], LOW_TOKEN_THRESHOLD);

        // The suffix array finds many overlapping sub-sequences, but only
        // the longest match should survive — shorter ones are fully contained.
        assert_eq!(groups.len(), 1);
    }
}
