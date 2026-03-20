use std::path::Path;

use super::rules::SimilarityRules;
use super::tree::{NodeKind, SourceTree, SourceTreeBuilder};
use crate::parser::{AstParser, TreeSitterParser};

#[derive(Debug)]
pub struct ParseError;

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to produce a parse tree")
    }
}

impl std::error::Error for ParseError {}

/// Parse source code into a [`SourceTree`] using trait-based language rules.
///
/// The walker is language-agnostic. It handles unnamed nodes generically
/// and delegates named node classification to the language rules.
///
/// # Errors
///
/// Returns `ParseError` if the parser fails to produce a parse tree.
pub fn parse_source(
    source: &str,
    path: &str,
    rules: &dyn SimilarityRules,
) -> Result<SourceTree, ParseError> {
    let mut parser = TreeSitterParser::new();
    let tree = parser
        .parse(source, &rules.language())
        .map_err(|_| ParseError)?;

    let mut builder = SourceTreeBuilder::new(path.to_string());

    let has_file_context = if let Some(kind) = rules.classify_file(Path::new(path)) {
        builder.open_container(kind);
        true
    } else {
        false
    };

    walk_node(tree.root_node(), source.as_bytes(), rules, &mut builder);

    if has_file_context {
        builder.close_container();
    }

    Ok(builder.build())
}

fn walk_node(
    node: tree_sitter::Node,
    src: &[u8],
    rules: &dyn SimilarityRules,
    builder: &mut SourceTreeBuilder,
) {
    if node.is_error() || node.is_missing() {
        return;
    }

    if !node.is_named() {
        return walk_leaf_or_recurse(node, node.kind().to_string(), src, rules, builder);
    }

    let Some(kind) = rules.classify_node(node, src) else {
        let text = node.utf8_text(src).unwrap_or("").to_string();
        return walk_leaf_or_recurse(node, text, src, rules, builder);
    };

    match kind {
        NodeKind::Token {
            text,
            start_line,
            end_line,
        } => builder.add_token(text, start_line, end_line),
        NodeKind::Comment | NodeKind::Decoration => {}
        container => {
            builder.open_container(container);
            walk_children(node, src, rules, builder);
            builder.close_container();
        }
    }
}

fn walk_leaf_or_recurse(
    node: tree_sitter::Node,
    text: String,
    src: &[u8],
    rules: &dyn SimilarityRules,
    builder: &mut SourceTreeBuilder,
) {
    if node.child_count() == 0 {
        builder.add_token(
            text,
            node.start_position().row + 1,
            node.end_position().row + 1,
        );
    } else {
        walk_children(node, src, rules, builder);
    }
}

fn walk_children(
    node: tree_sitter::Node,
    src: &[u8],
    rules: &dyn SimilarityRules,
    builder: &mut SourceTreeBuilder,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(child, src, rules, builder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_similarity::javascript::JsFamily;
    use crate::code_similarity::rust::Rust;

    #[test]
    fn rust_syntax_errors_do_not_panic() {
        assert!(parse_source("fn f(x: i32 -> { x + }", "broken.rs", &Rust).is_ok());
    }

    #[test]
    fn javascript_syntax_errors_do_not_panic() {
        assert!(
            parse_source(
                "function f(x { return +; }",
                "broken.js",
                &JsFamily::javascript()
            )
            .is_ok()
        );
    }

    #[test]
    fn typescript_syntax_errors_do_not_panic() {
        assert!(
            parse_source(
                "function f(x: number { return +; }",
                "broken.ts",
                &JsFamily::typescript()
            )
            .is_ok()
        );
    }

    #[test]
    fn syntax_errors_preserve_tokens_from_valid_parts() {
        let tree = parse_source("fn valid() { 1 + 2 }\nfn broken(x: { }", "a.rs", &Rust).unwrap();
        let tokens = tree.tokens();
        let texts: Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();

        assert!(
            texts.contains(&"fn"),
            "valid tokens should survive syntax errors, got: {texts:?}"
        );
        assert!(
            tokens.len() >= 5,
            "expected several tokens from the valid part"
        );
    }
}
