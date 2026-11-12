//! Node shapes that several tree-sitter grammars share.

use super::rules::{ElseBranch, LogicalExpression, LogicalOp};

/// Reads an `else_clause` node, which wraps the `else` keyword and what
/// follows it (tree-sitter-rust, tree-sitter-javascript, tree-sitter-typescript).
pub fn else_clause(clause: tree_sitter::Node) -> Option<ElseBranch> {
    let mut cursor = clause.walk();
    let keyword = clause.children(&mut cursor).find(|c| c.kind() == "else")?;
    let body = clause.named_children(&mut cursor).find(|c| !c.is_extra())?;
    Some(ElseBranch { keyword, body })
}

/// Reads a `binary_expression` node whose operator is `&&` or `||`
/// (tree-sitter-rust, tree-sitter-javascript, tree-sitter-typescript).
pub fn logical_binary_expression(node: tree_sitter::Node) -> Option<LogicalExpression> {
    Some(LogicalExpression {
        operator: logical_operator(node)?,
        left: logical_operand(node.child_by_field_name("left")?),
        right: logical_operand(node.child_by_field_name("right")?),
    })
}

fn logical_operator(node: tree_sitter::Node) -> Option<LogicalOp> {
    if node.kind() != "binary_expression" {
        return None;
    }
    match node.child_by_field_name("operator")?.kind() {
        "&&" => Some(LogicalOp::And("&&")),
        "||" => Some(LogicalOp::Or("||")),
        _ => None,
    }
}

/// Looks through the parentheses around an operand when they group another
/// logical expression. Any other operand stays as written.
fn logical_operand(node: tree_sitter::Node) -> tree_sitter::Node {
    let grouped = skip_parentheses(node);
    if logical_operator(grouped).is_some() {
        grouped
    } else {
        node
    }
}

/// Looks through `parenthesized_expression` nodes down to the expression they
/// group.
fn skip_parentheses(mut node: tree_sitter::Node) -> tree_sitter::Node {
    while node.kind() == "parenthesized_expression" {
        let Some(inner) = node
            .named_children(&mut node.walk())
            .find(|c| !c.is_extra())
        else {
            break;
        };
        node = inner;
    }
    node
}
