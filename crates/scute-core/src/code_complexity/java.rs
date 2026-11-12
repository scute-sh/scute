use crate::language::Java;

use super::rules::{
    ComplexityRules, Construct, ElseBranch, FlowConstruct, IfConstruct, JumpKeyword,
    LogicalExpression, NestingKind, ScoringUnit,
};

impl ComplexityRules for Java {
    fn scoring_unit<'a>(
        &self,
        node: tree_sitter::Node<'a>,
        src: &'a [u8],
    ) -> Option<ScoringUnit<'a>> {
        if node.kind() != "method_declaration" {
            return None;
        }
        let name = node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(src).ok())
            .unwrap_or("")
            .to_string();
        Some(ScoringUnit {
            name,
            line: node.start_position().row + 1,
            node: node.child_by_field_name("body").unwrap_or(node),
            receiver_type: None,
        })
    }

    fn if_construct<'a>(&self, node: tree_sitter::Node<'a>) -> Option<IfConstruct<'a>> {
        if node.kind() != "if_statement" {
            return None;
        }
        Some(IfConstruct {
            condition: node.child_by_field_name("condition")?,
            consequence: node.child_by_field_name("consequence")?,
            alternative: else_branch(node),
        })
    }

    fn flow_construct(&self, node: tree_sitter::Node) -> Option<FlowConstruct> {
        match node.kind() {
            "enhanced_for_statement" => Some(FlowConstruct {
                role: Construct::Loop,
                label: "for-each",
            }),
            _ => None,
        }
    }

    fn nesting_kind(&self, _node: tree_sitter::Node) -> Option<NestingKind> {
        None
    }

    fn logical_expression<'a>(
        &self,
        _node: tree_sitter::Node<'a>,
    ) -> Option<LogicalExpression<'a>> {
        None
    }

    fn jump_label(&self, _node: tree_sitter::Node, _src: &[u8]) -> Option<(JumpKeyword, String)> {
        None
    }

    fn is_recursive_call(
        &self,
        _node: tree_sitter::Node,
        _fn_name: &str,
        _receiver_type: Option<&str>,
        _src: &[u8],
    ) -> bool {
        false
    }
}

/// An `if_statement` holds its `else` keyword directly, with the statement
/// after it as its `alternative`.
fn else_branch(if_statement: tree_sitter::Node) -> Option<ElseBranch> {
    let body = if_statement.child_by_field_name("alternative")?;
    let keyword = if_statement
        .children(&mut if_statement.walk())
        .find(|c| c.kind() == "else")?;
    Some(ElseBranch { keyword, body })
}
