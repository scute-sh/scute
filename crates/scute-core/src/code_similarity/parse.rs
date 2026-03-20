use std::path::Path;

use super::rules::SimilarityRules;
use super::tree::{NodeKind, SourceTree, SourceTreeBuilder};
use crate::files::SourceFile;
use crate::language::LanguageRegistry;
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
    file: &SourceFile,
    languages: &LanguageRegistry<dyn SimilarityRules>,
) -> Result<SourceTree, ParseError> {
    let (lang, rules) = languages.for_path(&file.path).ok_or(ParseError)?;
    let mut parser = TreeSitterParser::new();
    let tree = parser
        .parse(&file.content, &lang.grammar)
        .map_err(|_| ParseError)?;

    let mut builder = SourceTreeBuilder::new(file.path.display().to_string());

    let has_file_context = if let Some(kind) = rules.classify_file(&file.path) {
        builder.open_container(kind);
        true
    } else {
        false
    };

    walk_node(
        tree.root_node(),
        file.content.as_bytes(),
        rules,
        &mut builder,
    );

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
    use crate::code_similarity::check::languages;
    use crate::files::SourceFile;

    fn parse(source: &str, path: &str) -> Result<SourceTree, ParseError> {
        parse_source(
            &SourceFile {
                path: path.into(),
                content: source.into(),
            },
            &languages(),
        )
    }

    #[test]
    fn rust_syntax_errors_do_not_panic() {
        assert!(parse("fn f(x: i32 -> { x + }", "broken.rs").is_ok());
    }

    #[test]
    fn javascript_syntax_errors_do_not_panic() {
        assert!(parse("function f(x { return +; }", "broken.js").is_ok());
    }

    #[test]
    fn typescript_syntax_errors_do_not_panic() {
        assert!(parse("function f(x: number { return +; }", "broken.ts").is_ok());
    }

    #[test]
    fn syntax_errors_preserve_tokens_from_valid_parts() {
        let tree = parse("fn valid() { 1 + 2 }\nfn broken(x: { }", "a.rs").unwrap();
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
