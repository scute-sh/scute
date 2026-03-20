use std::path::Path;

use super::tree::NodeKind;

/// Build a `NodeKind::Token` from a tree-sitter node, using 1-indexed lines.
#[must_use]
pub fn token(text: &str, node: &tree_sitter::Node) -> NodeKind {
    NodeKind::Token {
        text: text.to_string(),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
    }
}

/// Maps a language's tree-sitter AST to our source tree model.
///
/// The source tree builder is language-agnostic: it handles unnamed nodes
/// (operators, punctuation) and default recursion. This trait provides
/// the language-specific decisions: which named nodes are structural
/// containers, which are tokens, and which should be skipped.
pub trait SimilarityRules {
    /// File-level structural context based on path conventions.
    ///
    /// Returns a container kind to wrap the entire file's content in.
    fn classify_file(&self, path: &Path) -> Option<NodeKind>;

    /// Classify a named AST node.
    ///
    /// Returns `None` for nodes the language doesn't have an opinion
    /// about. The builder handles those with default behavior: recurse
    /// for internal nodes, emit text for leaves.
    fn classify_node(&self, node: tree_sitter::Node, src: &[u8]) -> Option<NodeKind>;
}
