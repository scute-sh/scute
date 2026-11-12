use crate::language::JsFamily;

use super::grammar::{else_clause, logical_binary_expression};
use super::rules::{
    ComplexityRules, Construct, FlowConstruct, IfConstruct, JumpKeyword, LogicalExpression,
    NestingKind, ScoringUnit,
};

impl ComplexityRules for JsFamily {
    fn scoring_unit<'a>(
        &self,
        node: tree_sitter::Node<'a>,
        src: &'a [u8],
    ) -> Option<ScoringUnit<'a>> {
        match node.kind() {
            "function_declaration" | "generator_function_declaration" | "method_definition" => {}
            _ => return None,
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

    fn is_recursive_call(
        &self,
        node: tree_sitter::Node,
        fn_name: &str,
        _receiver_type: Option<&str>,
        src: &[u8],
    ) -> bool {
        if node.kind() != "call_expression" {
            return false;
        }
        let Some(target) = node.child_by_field_name("function") else {
            return false;
        };
        callee_name(target, src) == Some(fn_name)
    }

    fn jump_label(&self, node: tree_sitter::Node, src: &[u8]) -> Option<(JumpKeyword, String)> {
        let keyword = match node.kind() {
            "break_statement" => JumpKeyword::Break,
            "continue_statement" => JumpKeyword::Continue,
            _ => return None,
        };
        let label = node
            .children(&mut node.walk())
            .find(|c| c.kind() == "statement_identifier")
            .and_then(|l| l.utf8_text(src).ok())?;
        Some((keyword, label.to_string()))
    }

    fn if_construct<'a>(&self, node: tree_sitter::Node<'a>) -> Option<IfConstruct<'a>> {
        if node.kind() != "if_statement" {
            return None;
        }
        Some(IfConstruct {
            condition: node.child_by_field_name("condition")?,
            consequence: node.child_by_field_name("consequence")?,
            alternative: node
                .child_by_field_name("alternative")
                .and_then(else_clause),
        })
    }

    fn flow_construct(&self, node: tree_sitter::Node) -> Option<FlowConstruct> {
        match node.kind() {
            "for_statement" => Some(FlowConstruct {
                role: Construct::Loop,
                label: "for",
            }),
            "for_in_statement" => Some(FlowConstruct {
                role: Construct::Loop,
                label: if has_child_kind(node, "of") {
                    "for...of"
                } else {
                    "for...in"
                },
            }),
            "while_statement" => Some(FlowConstruct {
                role: Construct::Loop,
                label: "while",
            }),
            "do_statement" => Some(FlowConstruct {
                role: Construct::Loop,
                label: "do...while",
            }),
            "switch_statement" => Some(FlowConstruct {
                role: Construct::Conditional,
                label: "switch",
            }),
            "catch_clause" => Some(FlowConstruct {
                role: Construct::ExceptionHandler,
                label: "catch",
            }),
            "ternary_expression" => Some(FlowConstruct {
                role: Construct::Conditional,
                label: "ternary",
            }),
            _ => None,
        }
    }

    fn nesting_kind(&self, node: tree_sitter::Node) -> Option<NestingKind> {
        match node.kind() {
            "arrow_function" => Some(NestingKind::Inline(FlowConstruct {
                role: Construct::InlineNesting,
                label: "arrow",
            })),
            "function_expression" => Some(NestingKind::Inline(FlowConstruct {
                role: Construct::InlineNesting,
                label: "function",
            })),
            "generator_function" => Some(NestingKind::Inline(FlowConstruct {
                role: Construct::InlineNesting,
                label: "generator",
            })),
            "function_declaration" | "generator_function_declaration" | "method_definition" => {
                Some(NestingKind::Separate)
            }
            _ => None,
        }
    }

    fn logical_expression<'a>(&self, node: tree_sitter::Node<'a>) -> Option<LogicalExpression<'a>> {
        logical_binary_expression(node)
    }
}

fn callee_name<'a>(target: tree_sitter::Node, src: &'a [u8]) -> Option<&'a str> {
    match target.kind() {
        "member_expression" => {
            let object = target.child_by_field_name("object")?;
            if object.kind() != "this" {
                return None;
            }
            target
                .child_by_field_name("property")
                .and_then(|n| n.utf8_text(src).ok())
        }
        _ => target.utf8_text(src).ok(),
    }
}

fn has_child_kind(node: tree_sitter::Node, kind: &str) -> bool {
    node.children(&mut node.walk()).any(|c| c.kind() == kind)
}
