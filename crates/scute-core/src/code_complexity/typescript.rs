use tree_sitter::Language;

use super::rules::{LanguageRules, NestingKind, ScoringUnit};
use super::score::{Construct, FlowConstruct, JumpKeyword, LogicalOp};

pub struct TypeScript {
    language: Language,
}

impl TypeScript {
    pub fn new(language: Language) -> Self {
        Self { language }
    }
}

impl LanguageRules for TypeScript {
    fn language(&self) -> Language {
        self.language.clone()
    }

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
        node.child_by_field_name("function")
            .and_then(|f| f.utf8_text(src).ok())
            == Some(fn_name)
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

    fn flow_construct(&self, node: tree_sitter::Node) -> Option<FlowConstruct> {
        match node.kind() {
            "if_statement" => Some(FlowConstruct {
                role: Construct::Conditional,
                label: "if",
            }),
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
            "function_declaration" | "generator_function_declaration" => {
                Some(NestingKind::Separate)
            }
            _ => None,
        }
    }

    fn is_else_clause(&self, node: tree_sitter::Node) -> bool {
        node.kind() == "else_clause"
    }

    fn is_else_if(&self, node: tree_sitter::Node) -> bool {
        node.children(&mut node.walk())
            .any(|c| c.kind() == "if_statement")
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
}

fn has_child_kind(node: tree_sitter::Node, kind: &str) -> bool {
    node.children(&mut node.walk()).any(|c| c.kind() == kind)
}
