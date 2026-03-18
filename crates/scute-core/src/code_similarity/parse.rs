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
        return walk_unnamed(node, src, rules, builder);
    }

    let Some(kind) = rules.classify_node(node, src) else {
        return walk_unclassified(node, src, rules, builder);
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

/// Named node the language didn't classify: emit as token or recurse.
fn walk_unclassified(
    node: tree_sitter::Node,
    src: &[u8],
    rules: &dyn SimilarityRules,
    builder: &mut SourceTreeBuilder,
) {
    if node.child_count() == 0 {
        let text = node.utf8_text(src).unwrap_or("");
        builder.add_token(
            text.to_string(),
            node.start_position().row + 1,
            node.end_position().row + 1,
        );
    } else {
        walk_children(node, src, rules, builder);
    }
}

fn walk_unnamed(
    node: tree_sitter::Node,
    src: &[u8],
    rules: &dyn SimilarityRules,
    builder: &mut SourceTreeBuilder,
) {
    if node.child_count() == 0 {
        builder.add_token(
            node.kind().to_string(),
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
    use crate::code_similarity::rust::Rust;

    #[test]
    fn syntax_errors_do_not_panic() {
        let result = parse_source("fn f(x: i32 -> { x + }", "broken.rs", &Rust);

        assert!(result.is_ok()); // tree-sitter recovers, never errors
    }
}
