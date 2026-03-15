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

    fn score_only(source: &str) -> u64 {
        let results = score_functions(source, &Rust);
        assert_eq!(results.len(), 1, "expected exactly one function");
        results[0].score
    }

    #[test_case("fn add(a: i32, b: i32) -> i32 { a + b }", 0 ; "scores_zero_for_flat_function")]
    #[test_case("fn f(x: i32) -> bool { if x > 0 { return true; } false }", 1 ; "scores_one_for_single_if")]
    #[test_case("fn f(a: bool, b: bool, c: bool) -> bool { a && b && c }", 1 ; "scores_one_for_same_logical_operators")]
    #[test_case("fn f(a: bool, b: bool, c: bool) -> bool { a && b || c }", 2 ; "scores_per_operator_change")]
    // if: +1, else if: +1 (flat chain), else: +1
    #[test_case("fn f(x: i32) -> i32 {
        if x > 0 { 1 }
        else if x < 0 { -1 }
        else { 0 }
    }", 3 ; "scores_else_if_chain_flat")]
    // if: +1, else: +1, recursion: +1
    #[test_case("fn factorial(n: u64) -> u64 {
        if n <= 1 { 1 }
        else { n * factorial(n - 1) }
    }", 3 ; "adds_one_for_direct_recursion")]
    // closure: +0 structural, nesting becomes 1; if: +1+1, else: +1
    #[test_case("fn f(items: &[i32]) -> Vec<i32> {
        items.iter().filter(|x| {
            if **x > 0 { true } else { false }
        }).copied().collect()
    }", 3 ; "increases_nesting_for_closure")]
    // for+nested-if+else = 1 + (1+1) + (1+2) + 1
    #[test_case("fn process(items: &[i32]) -> i32 {
        let mut total = 0;
        for item in items {
            if *item > 0 {
                if *item > 10 { total += item; }
                else { total -= item; }
            }
        }
        total
    }", 7 ; "scores_canonical_example")]
    // outer for: +1, inner for: +2, if: +3, break 'outer: +1
    #[test_case("fn f(items: &[&[i32]]) -> i32 {
        let mut total = 0;
        'outer: for row in items {
            for item in *row {
                if *item < 0 { break 'outer; }
                total += item;
            }
        }
        total
    }", 7 ; "adds_one_for_labeled_break")]
    // impl method should be discovered and scored
    #[test_case("struct S;
    impl S {
        fn method(&self, x: i32) -> i32 {
            if x > 0 { 1 } else { -1 }
        }
    }", 2 ; "scores_impl_method")]
    // if: +1, else: +1, recursion via self.method(): +1
    #[test_case("struct S;
    impl S {
        fn count(&self, n: u64) -> u64 {
            if n <= 1 { 1 }
            else { n * self.count(n - 1) }
        }
    }", 3 ; "adds_one_for_self_method_recursion")]
    // if: +1, else: +1, recursion via Self::method(): +1
    #[test_case("struct S;
    impl S {
        fn count(n: u64) -> u64 {
            if n <= 1 { 1 }
            else { n * Self::count(n - 1) }
        }
    }", 3 ; "adds_one_for_associated_function_recursion")]
    // if: +1, else: +1 — Def::foo is NOT recursion into Abc::foo
    #[test_case("struct Abc;
    struct Def;
    impl Abc {
        fn foo(n: u64) -> u64 {
            if n <= 1 { 1 }
            else { Def::foo(n - 1) }
        }
    }", 2 ; "different_type_qualified_call_is_not_recursion")]
    // else if with nested if inside the else-if branch (nesting underflow regression)
    #[test_case("fn f(x: i32, y: bool) -> i32 {
        if x > 0 { 1 }
        else if y { if x < -10 { 2 } else { 3 } }
        else { 0 }
    }", 6 ; "scores_nested_if_inside_else_if")]
    fn scores(source: &str, expected: u64) {
        assert_eq!(score_only(source), expected);
    }

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

    // outer: inner fn bumps nesting to 1, if inside inner at nesting 1 = +2, if in outer = +1 → 3
    // inner: scored independently, if at nesting 0 = +1 → 1
    #[test]
    fn scores_nested_function_and_outer_independently() {
        let results = score_functions(
            "fn outer() {
                fn inner() { if true {} }
                if true {}
            }",
            &Rust,
        );
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "outer");
        assert_eq!(results[0].score, 3);
        assert_eq!(results[1].name, "inner");
        assert_eq!(results[1].score, 1);
    }

    mod typescript {
        use super::*;
        use crate::code_complexity::typescript::TypeScript;
        use test_case::test_case;

        fn ts_rules() -> TypeScript {
            TypeScript::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        }

        fn ts_score(source: &str) -> u64 {
            let results = score_functions(source, &ts_rules());
            assert_eq!(results.len(), 1, "expected exactly one function");
            results[0].score
        }

        #[test_case("function f(x: number) { if (x > 0) { return x; } }", 1 ; "scores_if")]
        #[test_case("function f(items: number[]) { for (let i = 0; i < items.length; i++) {} }", 1 ; "scores_for")]
        #[test_case("function f(obj: any) { for (const k in obj) {} }", 1 ; "scores_for_in")]
        #[test_case("function f(items: number[]) { for (const x of items) {} }", 1 ; "scores_for_of")]
        #[test_case("function f(x: number) { while (x > 0) { x--; } }", 1 ; "scores_while")]
        #[test_case("function f(x: number) { do { x--; } while (x > 0); }", 1 ; "scores_do_while")]
        #[test_case("function f(x: number) { switch (x) { case 1: break; } }", 1 ; "scores_switch")]
        #[test_case("function f() { try {} catch (e) {} }", 1 ; "scores_catch")]
        #[test_case("function f(x: boolean) { return x ? 1 : 0; }", 1 ; "scores_ternary")]
        fn structural_increments(source: &str, expected: u64) {
            assert_eq!(ts_score(source), expected);
        }

        // if: +1, else: +1
        #[test_case("function f(x: number) {
            if (x > 0) { return 1; }
            else { return -1; }
        }", 2 ; "scores_else_branch")]
        // if: +1, else if: +1 (flat), else: +1
        #[test_case("function f(x: number) {
            if (x > 0) { return 1; }
            else if (x < 0) { return -1; }
            else { return 0; }
        }", 3 ; "scores_else_if_chain_flat")]
        fn else_and_else_if(source: &str, expected: u64) {
            assert_eq!(ts_score(source), expected);
        }

        #[test_case("function f(a: boolean, b: boolean, c: boolean) { return a && b && c; }", 1 ; "scores_same_logical_operators")]
        #[test_case("function f(a: boolean, b: boolean, c: boolean) { return a && b || c; }", 2 ; "scores_mixed_logical_operators")]
        #[test_case("function f(a: any, b: any) { return a ?? b; }", 0 ; "ignores_nullish_coalescing")]
        fn logical_operators(source: &str, expected: u64) {
            assert_eq!(ts_score(source), expected);
        }

        // outer for: +1, inner for: +2, if: +3, break outer: +1
        #[test]
        fn adds_one_for_labeled_break() {
            assert_eq!(
                ts_score(
                    "function f(matrix: number[][]) {
                        let total = 0;
                        outer: for (const row of matrix) {
                            for (const item of row) {
                                if (item < 0) { break outer; }
                                total += item;
                            }
                        }
                        return total;
                    }"
                ),
                7
            );
        }

        // arrow: nesting +1, if: +1+1, else: +1
        #[test]
        fn arrow_function_increases_nesting() {
            assert_eq!(
                ts_score(
                    "function f(items: number[]) {
                        return items.filter((x) => {
                            if (x > 0) { return true; }
                            else { return false; }
                        });
                    }"
                ),
                3
            );
        }

        // if: +1, else: +1, recursion: +1
        #[test]
        fn adds_one_for_direct_recursion() {
            assert_eq!(
                ts_score(
                    "function factorial(n: number): number {
                        if (n <= 1) { return 1; }
                        else { return n * factorial(n - 1); }
                    }"
                ),
                3
            );
        }

        // outer: nested fn bumps nesting to 1, if at nesting 1 = +2, if in outer = +1 → 3
        // inner: scored independently, if at nesting 0 = +1 → 1
        #[test]
        fn scores_nested_named_function_independently() {
            let results = score_functions(
                "function outer() {
                    function inner() { if (true) {} }
                    if (true) {}
                }",
                &ts_rules(),
            );
            assert_eq!(results.len(), 2);
            assert_eq!(results[0].name, "outer");
            assert_eq!(results[0].score, 3);
            assert_eq!(results[1].name, "inner");
            assert_eq!(results[1].score, 1);
        }

        #[test]
        fn scores_class_methods_independently() {
            let results = score_functions(
                "class Calc {
                    add(a: number, b: number) { return a + b; }
                    check(x: number) { if (x > 0) { return true; } return false; }
                }",
                &ts_rules(),
            );
            assert_eq!(results.len(), 2);
            assert_eq!(results[0].name, "add");
            assert_eq!(results[0].score, 0);
            assert_eq!(results[1].name, "check");
            assert_eq!(results[1].score, 1);
        }
    }
}
