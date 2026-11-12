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

/// Reads a `binary_expression` node whose operator is `&&` or `||`, looking
/// through the parentheses around its operands (tree-sitter-rust,
/// tree-sitter-javascript, tree-sitter-typescript).
pub fn logical_binary_expression(node: tree_sitter::Node) -> Option<LogicalExpression> {
    if node.kind() != "binary_expression" {
        return None;
    }
    let operator = match node.child_by_field_name("operator")?.kind() {
        "&&" => LogicalOp::And("&&"),
        "||" => LogicalOp::Or("||"),
        _ => return None,
    };
    Some(LogicalExpression {
        operator,
        left: skip_parentheses(node.child_by_field_name("left")?),
        right: skip_parentheses(node.child_by_field_name("right")?),
    })
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
