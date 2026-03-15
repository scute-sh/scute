use tree_sitter::Language;

use super::rules::{LanguageRules, NestingKind, ScoringUnit};
use super::score::{Construct, FlowConstruct, JumpKeyword, LogicalOp};

pub struct Rust;

impl LanguageRules for Rust {
    fn language(&self) -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn scoring_unit<'a>(
        &self,
        node: tree_sitter::Node<'a>,
        src: &'a [u8],
    ) -> Option<ScoringUnit<'a>> {
        if node.kind() != "function_item" {
            return None;
        }
        let name = node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(src).ok())
            .unwrap_or("")
            .to_string();
        let receiver_type = enclosing_impl_type(node, src);
        Some(ScoringUnit {
            name,
            line: node.start_position().row + 1,
            node,
            receiver_type,
        })
    }

    fn flow_construct(&self, node: tree_sitter::Node) -> Option<FlowConstruct> {
        match node.kind() {
            "if_expression" => Some(FlowConstruct {
                role: Construct::Conditional,
                label: "if",
            }),
            "for_expression" => Some(FlowConstruct {
                role: Construct::Loop,
                label: "for",
            }),
            "while_expression" => Some(FlowConstruct {
                role: Construct::Loop,
                label: "while",
            }),
            "loop_expression" => Some(FlowConstruct {
                role: Construct::Loop,
                label: "loop",
            }),
            "match_expression" => Some(FlowConstruct {
                role: Construct::Conditional,
                label: "match",
            }),
            _ => None,
        }
    }

    fn is_else_clause(&self, node: tree_sitter::Node) -> bool {
        node.kind() == "else_clause"
    }

    fn is_else_if(&self, node: tree_sitter::Node) -> bool {
        node.children(&mut node.walk())
            .any(|c| c.kind() == "if_expression")
    }

    fn nesting_kind(&self, node: tree_sitter::Node) -> Option<NestingKind> {
        match node.kind() {
            "closure_expression" => Some(NestingKind::Inline(FlowConstruct {
                role: Construct::InlineNesting,
                label: "closure",
            })),
            "function_item" => Some(NestingKind::Separate),
            _ => None,
        }
    }

    fn logical_operator(&self, node: tree_sitter::Node) -> Option<LogicalOp> {
        if node.kind() != "binary_expression" {
            return None;
        }
        node.children(&mut node.walk())
            .find(|c| c.kind() == "&&" || c.kind() == "||")
            .map(|c| match c.kind() {
                "&&" => LogicalOp::And("&&"),
                _ => LogicalOp::Or("||"),
            })
    }

    fn is_logical_operator_token(&self, node: tree_sitter::Node) -> bool {
        node.kind() == "&&" || node.kind() == "||"
    }

    fn jump_label(&self, node: tree_sitter::Node, src: &[u8]) -> Option<(JumpKeyword, String)> {
        let keyword = match node.kind() {
            "break_expression" => JumpKeyword::Break,
            "continue_expression" => JumpKeyword::Continue,
            _ => return None,
        };
        let label = node
            .children(&mut node.walk())
            .find(|c| c.kind() == "label")
            .and_then(|l| l.utf8_text(src).ok())?;
        Some((keyword, label.to_string()))
    }

    fn is_recursive_call(
        &self,
        node: tree_sitter::Node,
        fn_name: &str,
        receiver_type: Option<&str>,
        src: &[u8],
    ) -> bool {
        if node.kind() != "call_expression" {
            return false;
        }
        let Some(target) = node.child_by_field_name("function") else {
            return false;
        };
        callee_name(target, src) == Some(fn_name) && scope_is_self(target, receiver_type, src)
    }
}

fn enclosing_impl_type(node: tree_sitter::Node, src: &[u8]) -> Option<String> {
    std::iter::successors(node.parent(), tree_sitter::Node::parent)
        .find(|n| n.kind() == "impl_item")
        .and_then(|n| n.child_by_field_name("type"))
        .and_then(|t| t.utf8_text(src).ok())
        .map(String::from)
}

fn callee_name<'a>(target: tree_sitter::Node, src: &'a [u8]) -> Option<&'a str> {
    match target.kind() {
        "field_expression" => field_text(target, "field", src), // self.foo()
        "scoped_identifier" => field_text(target, "name", src), // Self::foo()
        _ => target.utf8_text(src).ok(),                        // foo()
    }
}

fn scope_is_self(target: tree_sitter::Node, impl_type: Option<&str>, src: &[u8]) -> bool {
    if target.kind() != "scoped_identifier" {
        return true;
    }
    field_text(target, "path", src).is_some_and(|scope| scope == "Self" || impl_type == Some(scope))
}

fn field_text<'a>(node: tree_sitter::Node, field: &str, src: &'a [u8]) -> Option<&'a str> {
    node.child_by_field_name(field)
        .and_then(|n| n.utf8_text(src).ok())
}
