use crate::language::JsFamily;

use super::grammar::else_clause;
use super::rules::{
    ComplexityRules, Construct, FlowConstruct, IfConstruct, JumpKeyword, LogicalOp, NestingKind,
    ScoringUnit,
};

impl ComplexityRules for JsFamily {
    fn scoring_unit<'a>(
        &self,
        node: tree_sitter::Node<'a>,
        src: &'a [u8],
    ) -> Option<ScoringUnit<'a>> {
        let (name, function) = if DECLARED_FUNCTIONS.contains(&node.kind()) {
            (node.child_by_field_name("name"), node)
        } else {
            assigned_function(node)?
        };
        Some(ScoringUnit {
            name: name
                .and_then(|n| n.utf8_text(src).ok())
                .unwrap_or("")
                .to_string(),
            line: function.start_position().row + 1,
            node: function.child_by_field_name("body").unwrap_or(function),
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

const DECLARED_FUNCTIONS: &[&str] = &[
    "function_declaration",
    "generator_function_declaration",
    "method_definition",
];
const FUNCTION_EXPRESSIONS: &[&str] = &[
    "arrow_function",
    "function_expression",
    "generator_function",
];

/// A function expression bound to a name outside any other function: a
/// variable, an object-literal property or a class field. Returns the name
/// node and the function.
fn assigned_function(
    node: tree_sitter::Node,
) -> Option<(Option<tree_sitter::Node>, tree_sitter::Node)> {
    let name_field = match node.kind() {
        "variable_declarator" | "public_field_definition" => "name",
        "pair" => "key",
        "field_definition" => "property",
        _ => return None,
    };
    let function = node
        .child_by_field_name("value")
        .filter(|value| FUNCTION_EXPRESSIONS.contains(&value.kind()))?;
    if is_inside_function(node) {
        return None;
    }
    Some((node.child_by_field_name(name_field), function))
}

fn is_inside_function(node: tree_sitter::Node) -> bool {
    std::iter::successors(node.parent(), tree_sitter::Node::parent).any(|ancestor| {
        DECLARED_FUNCTIONS.contains(&ancestor.kind())
            || FUNCTION_EXPRESSIONS.contains(&ancestor.kind())
    })
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
