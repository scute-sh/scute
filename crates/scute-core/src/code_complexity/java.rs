use crate::language::Java;

use super::rules::{ComplexityRules, ElseBranch, NestingKind, ScoringUnit};
use super::score::{Construct, FlowConstruct, JumpKeyword, LogicalOp};

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

    fn flow_construct(&self, node: tree_sitter::Node) -> Option<FlowConstruct> {
        match node.kind() {
            "if_statement" => Some(FlowConstruct {
                role: Construct::Conditional,
                label: "if",
            }),
            "enhanced_for_statement" => Some(FlowConstruct {
                role: Construct::Loop,
                label: "for-each",
            }),
            _ => None,
        }
    }

    fn else_branch<'a>(&self, _conditional: tree_sitter::Node<'a>) -> Option<ElseBranch<'a>> {
        None
    }

    fn nesting_kind(&self, _node: tree_sitter::Node) -> Option<NestingKind> {
        None
    }

    fn logical_operator(&self, _node: tree_sitter::Node) -> Option<LogicalOp> {
        None
    }

    fn is_logical_operator_token(&self, _node: tree_sitter::Node) -> bool {
        false
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
