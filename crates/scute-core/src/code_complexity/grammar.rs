//! Node shapes that several tree-sitter grammars share.

use super::rules::ElseBranch;

/// Reads an `else_clause` node, which wraps the `else` keyword and what
/// follows it (tree-sitter-rust, tree-sitter-javascript, tree-sitter-typescript).
pub fn else_clause(clause: tree_sitter::Node) -> Option<ElseBranch> {
    let mut cursor = clause.walk();
    let keyword = clause.children(&mut cursor).find(|c| c.kind() == "else")?;
    let body = clause.named_children(&mut cursor).find(|c| !c.is_extra())?;
    Some(ElseBranch { keyword, body })
}
