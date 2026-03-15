use crate::parser::{AstParser, TreeSitterParser};

use super::rules::{LanguageRules, NestingKind, NodeRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Construct {
    Conditional,
    Loop,
    ExceptionHandler,
    InlineNesting,
}

impl Construct {
    pub fn flow_break_category(self) -> &'static str {
        match self {
            Self::Conditional => "conditional",
            Self::Loop => "loop",
            Self::ExceptionHandler => "exception handler",
            Self::InlineNesting => "closure",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowConstruct {
    pub role: Construct,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpKeyword {
    Break,
    Continue,
}

impl JumpKeyword {
    pub fn label(self) -> &'static str {
        match self {
            Self::Break => "break",
            Self::Continue => "continue",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LogicalOp {
    And(&'static str),
    Or(&'static str),
}

impl LogicalOp {
    pub fn label(self) -> &'static str {
        match self {
            Self::And(s) | Self::Or(s) => s,
        }
    }
}

impl PartialEq for LogicalOp {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::And(_), Self::And(_)) | (Self::Or(_), Self::Or(_))
        )
    }
}

impl Eq for LogicalOp {}

#[derive(Debug, Clone, PartialEq)]
pub enum ContributorKind {
    FlowBreak {
        construct: FlowConstruct,
    },
    Nesting {
        construct: FlowConstruct,
        depth: u64,
        chain: Vec<FlowConstruct>,
    },
    Else,
    Logical {
        operators: Vec<LogicalOp>,
    },
    Recursion {
        fn_name: String,
    },
    Jump {
        keyword: JumpKeyword,
        label: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Contributor {
    pub kind: ContributorKind,
    pub line: usize,
    pub increment: u64,
}

pub struct FunctionScore {
    pub name: String,
    pub line: usize,
    pub score: u64,
    pub contributors: Vec<Contributor>,
}

pub fn score_functions(source: &str, rules: &dyn LanguageRules) -> Vec<FunctionScore> {
    let mut parser = TreeSitterParser::new();
    let Ok(tree) = parser.parse(source, &rules.language()) else {
        return vec![];
    };

    let src = source.as_bytes();
    let mut results = vec![];
    collect_functions(tree.root_node(), src, rules, &mut results);
    results
}

fn collect_functions(
    node: tree_sitter::Node,
    src: &[u8],
    rules: &dyn LanguageRules,
    results: &mut Vec<FunctionScore>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(unit) = rules.scoring_unit(child, src) {
            let mut contributors = vec![];
            let score = ScoringContext {
                rules,
                fn_name: &unit.name,
                impl_type: unit.receiver_type.as_deref(),
                src,
                contributors: &mut contributors,
            }
            .complexity(unit.node, 0);
            results.push(FunctionScore {
                name: unit.name,
                line: unit.line,
                score,
                contributors,
            });
        }
        collect_functions(child, src, rules, results);
    }
}

fn classify(
    rules: &dyn LanguageRules,
    node: tree_sitter::Node,
    fn_name: &str,
    receiver_type: Option<&str>,
    src: &[u8],
) -> Option<NodeRole> {
    classify_structural(rules, node)
        .or_else(|| classify_contextual(rules, node, fn_name, receiver_type, src))
}

fn classify_structural(rules: &dyn LanguageRules, node: tree_sitter::Node) -> Option<NodeRole> {
    if let Some(construct) = rules.flow_construct(node) {
        return Some(NodeRole::FlowConstruct(construct));
    }
    if rules.is_else_clause(node) {
        return Some(NodeRole::ElseClause);
    }
    if rules.nesting_kind(node).is_some() {
        return Some(NodeRole::NestingBoundary);
    }
    if rules.logical_operator(node).is_some() {
        return Some(NodeRole::LogicalExpression);
    }
    None
}

fn classify_contextual(
    rules: &dyn LanguageRules,
    node: tree_sitter::Node,
    fn_name: &str,
    receiver_type: Option<&str>,
    src: &[u8],
) -> Option<NodeRole> {
    if let Some((keyword, label)) = rules.jump_label(node, src) {
        return Some(NodeRole::LabeledJump(keyword, label));
    }
    if rules.is_recursive_call(node, fn_name, receiver_type, src) {
        return Some(NodeRole::RecursiveCall);
    }
    None
}

struct ScoringContext<'a> {
    rules: &'a dyn LanguageRules,
    fn_name: &'a str,
    impl_type: Option<&'a str>,
    src: &'a [u8],
    contributors: &'a mut Vec<Contributor>,
}

impl ScoringContext<'_> {
    fn complexity(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .map(|child| self.score_node(child, nesting))
            .sum()
    }

    fn score_node(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        let Some(role) = classify(self.rules, node, self.fn_name, self.impl_type, self.src) else {
            return self.complexity(node, nesting);
        };
        match role {
            NodeRole::FlowConstruct(construct) => self.score_flow_break(node, nesting, construct),
            NodeRole::ElseClause => self.score_else(node, nesting),
            NodeRole::NestingBoundary => self.complexity(node, nesting + 1),
            NodeRole::LogicalExpression => self.score_logical_sequence(node, nesting),
            NodeRole::LabeledJump(keyword, label) => self.score_jump(node, nesting, keyword, label),
            NodeRole::RecursiveCall => self.score_recursion(node, nesting),
        }
    }

    fn push(&mut self, kind: ContributorKind, node: tree_sitter::Node, increment: u64) {
        self.contributors.push(Contributor {
            kind,
            line: node.start_position().row + 1,
            increment,
        });
    }

    fn score_flow_break(
        &mut self,
        node: tree_sitter::Node,
        nesting: u64,
        construct: FlowConstruct,
    ) -> u64 {
        let increment = 1 + nesting;
        let kind = if nesting > 0 {
            let mut chain = nesting_chain(node, self.rules);
            chain.push(construct);
            ContributorKind::Nesting {
                construct,
                depth: nesting,
                chain,
            }
        } else {
            ContributorKind::FlowBreak { construct }
        };
        self.push(kind, node, increment);
        increment + self.complexity(node, nesting + 1)
    }

    fn score_else(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        if self.rules.is_else_if(node) {
            self.complexity(node, nesting.saturating_sub(1))
        } else {
            self.push(ContributorKind::Else, node, 1);
            1 + self.complexity(node, nesting)
        }
    }

    fn score_jump(
        &mut self,
        node: tree_sitter::Node,
        nesting: u64,
        keyword: JumpKeyword,
        label: String,
    ) -> u64 {
        self.push(ContributorKind::Jump { keyword, label }, node, 1);
        1 + self.complexity(node, nesting)
    }

    fn score_recursion(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        self.push(
            ContributorKind::Recursion {
                fn_name: self.fn_name.to_string(),
            },
            node,
            1,
        );
        1 + self.complexity(node, nesting)
    }

    fn score_logical_sequence(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        let mut operators = vec![];
        collect_logical_operators(node, self.rules, &mut operators);

        let score = count_operator_sequences(&operators);
        if score > 0 {
            self.push(ContributorKind::Logical { operators }, node, score);
        }

        score + self.visit_logical_leaves(node, nesting)
    }

    fn visit_logical_leaves(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        let mut cursor = node.walk();
        let children: Vec<_> = node
            .children(&mut cursor)
            .filter(|child| !self.rules.is_logical_operator_token(*child))
            .collect();

        children
            .into_iter()
            .map(|child| {
                if self.rules.logical_operator(child).is_some() {
                    self.visit_logical_leaves(child, nesting)
                } else {
                    self.complexity(child, nesting)
                }
            })
            .sum()
    }
}

fn nesting_chain(node: tree_sitter::Node, rules: &dyn LanguageRules) -> Vec<FlowConstruct> {
    let ancestors = std::iter::successors(node.parent(), tree_sitter::Node::parent);
    let mut chain = Vec::new();
    for ancestor in ancestors {
        if let Some(construct) = rules.flow_construct(ancestor) {
            chain.push(construct);
            continue;
        }
        match rules.nesting_kind(ancestor) {
            Some(NestingKind::Inline(construct)) => {
                chain.push(construct);
                break;
            }
            Some(NestingKind::Separate) => break,
            None => {}
        }
    }
    chain.reverse();
    chain
}

fn count_operator_sequences(operators: &[LogicalOp]) -> u64 {
    operators
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .count() as u64
        + u64::from(!operators.is_empty())
}

fn collect_logical_operators(
    node: tree_sitter::Node,
    rules: &dyn LanguageRules,
    operators: &mut Vec<LogicalOp>,
) {
    let Some(op) = rules.logical_operator(node) else {
        return;
    };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if rules.logical_operator(child).is_some() {
            collect_logical_operators(child, rules, operators);
        }
    }
    operators.push(op);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_complexity::rust::Rust;
    use test_case::test_case;

    fn contributors(source: &str) -> Vec<Contributor> {
        let results = score_functions(source, &Rust);
        assert_eq!(results.len(), 1, "expected exactly one function");
        results.into_iter().next().unwrap().contributors
    }

    #[test]
    fn flat_function_has_no_contributors() {
        assert!(contributors("fn f() { 1 + 2 }").is_empty());
    }

    #[test]
    fn flow_break_at_depth_zero() {
        assert_eq!(
            contributors("fn f() { if true {} }"),
            vec![Contributor {
                kind: ContributorKind::FlowBreak {
                    construct: FlowConstruct {
                        role: Construct::Conditional,
                        label: "if",
                    },
                },
                line: 1,
                increment: 1,
            }]
        );
    }

    #[test]
    fn nesting_tracks_depth_and_chain() {
        let cs = contributors("fn f() { for x in [1] { if true {} } }");

        assert_eq!(
            cs[1],
            Contributor {
                kind: ContributorKind::Nesting {
                    construct: FlowConstruct {
                        role: Construct::Conditional,
                        label: "if",
                    },
                    depth: 1,
                    chain: vec![
                        FlowConstruct {
                            role: Construct::Loop,
                            label: "for"
                        },
                        FlowConstruct {
                            role: Construct::Conditional,
                            label: "if"
                        },
                    ],
                },
                line: 1,
                increment: 2,
            }
        );
    }

    #[test]
    fn else_appears_as_contributor() {
        let cs = contributors("fn f(x: bool) { if x {} else {} }");
        let else_c = cs.iter().find(|c| c.kind == ContributorKind::Else);

        assert_eq!(
            else_c,
            Some(&Contributor {
                kind: ContributorKind::Else,
                line: 1,
                increment: 1,
            })
        );
    }

    #[test]
    fn logical_contributor_captures_operators() {
        assert_eq!(
            contributors("fn f(a: bool, b: bool, c: bool) -> bool { a && b || c }"),
            vec![Contributor {
                kind: ContributorKind::Logical {
                    operators: vec![LogicalOp::And("&&"), LogicalOp::Or("||")],
                },
                line: 1,
                increment: 2,
            },]
        );
    }

    #[test]
    fn recursion_contributor_captures_function_name() {
        let cs = contributors("fn go(n: u64) -> u64 { go(n - 1) }");

        assert_eq!(
            cs,
            vec![Contributor {
                kind: ContributorKind::Recursion {
                    fn_name: "go".into()
                },
                line: 1,
                increment: 1,
            }]
        );
    }

    #[test_case(
        "fn f() { 'outer: loop { break 'outer; } }",
        JumpKeyword::Break
        ; "break_captures_keyword_and_label"
    )]
    #[test_case(
        "fn f() { 'outer: loop { continue 'outer; } }",
        JumpKeyword::Continue
        ; "continue_captures_keyword_and_label"
    )]
    fn jump_contributor(source: &str, expected_keyword: JumpKeyword) {
        let cs = contributors(source);
        let jump = cs
            .iter()
            .find(|c| matches!(c.kind, ContributorKind::Jump { .. }));

        assert_eq!(
            jump,
            Some(&Contributor {
                kind: ContributorKind::Jump {
                    keyword: expected_keyword,
                    label: "'outer".into(),
                },
                line: 1,
                increment: 1,
            })
        );
    }

    #[test]
    fn nesting_chain_includes_closure_boundary() {
        let cs =
            contributors("fn f() { for x in [1] { [1].iter().filter(|y| { if **y > 0 {} }); } }");
        let nested = cs
            .iter()
            .find(|c| matches!(c.kind, ContributorKind::Nesting { .. }));

        assert_eq!(
            nested,
            Some(&Contributor {
                kind: ContributorKind::Nesting {
                    construct: FlowConstruct {
                        role: Construct::Conditional,
                        label: "if",
                    },
                    depth: 2,
                    chain: vec![
                        FlowConstruct {
                            role: Construct::InlineNesting,
                            label: "closure"
                        },
                        FlowConstruct {
                            role: Construct::Conditional,
                            label: "if"
                        },
                    ],
                },
                line: 1,
                increment: 3,
            })
        );
    }

    // else if with nested if inside the else-if branch (nesting underflow regression)
    #[test]
    fn scores_nested_if_inside_else_if() {
        let results = score_functions(
            "fn f(x: i32, y: bool) -> i32 {
                if x > 0 { 1 }
                else if y { if x < -10 { 2 } else { 3 } }
                else { 0 }
            }",
            &Rust,
        );
        assert_eq!(results[0].score, 6);
    }

    // for+nested-if+else = 1 + (1+1) + (1+2) + 1
    #[test]
    fn scores_canonical_nested_example() {
        let results = score_functions(
            "fn process(items: &[i32]) -> i32 {
                let mut total = 0;
                for item in items {
                    if *item > 0 {
                        if *item > 10 { total += item; }
                        else { total -= item; }
                    }
                }
                total
            }",
            &Rust,
        );
        assert_eq!(results[0].score, 7);
    }

    #[test]
    fn empty_source_returns_no_functions() {
        let results = score_functions("", &Rust);
        assert!(results.is_empty());
    }

    #[test]
    fn broken_syntax_does_not_panic() {
        let results = score_functions("fn f(x: i32 -> { x + }", &Rust);
        // tree-sitter recovers from errors — should not panic
        let _ = results;
    }
}
