use crate::parser::{AstParser, TreeSitterParser};
use tree_sitter::Language;

/// Maps a language's tree-sitter AST to cognitive complexity drivers.
///
/// The scoring algorithm is language-agnostic: it asks the language to identify
/// its own constructs (flow control, nesting boundaries, logical operators, etc.)
/// and applies the Sonar cognitive complexity rules uniformly.
pub trait LanguageRules {
    fn language(&self) -> Language;
}

pub struct Rust;

impl LanguageRules for Rust {
    fn language(&self) -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Construct {
    If,
    For,
    While,
    Loop,
    Match,
    Closure,
}

impl Construct {
    pub fn label(self) -> &'static str {
        match self {
            Self::If => "if",
            Self::For => "for",
            Self::While => "while",
            Self::Loop => "loop",
            Self::Match => "match",
            Self::Closure => "closure",
        }
    }

    pub fn flow_break_label(self) -> &'static str {
        match self {
            Self::For | Self::While | Self::Loop => "loop",
            Self::If => "conditional",
            Self::Match => "expression",
            Self::Closure => "closure",
        }
    }
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

#[derive(Debug, Clone, PartialEq)]
pub enum ContributorKind {
    FlowBreak {
        construct: Construct,
    },
    Nesting {
        construct: Construct,
        depth: u64,
        chain: Vec<Construct>,
    },
    Else,
    Logical {
        operators: Vec<String>,
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
    collect_functions(tree.root_node(), src, &mut results);
    results
}

fn collect_functions(node: tree_sitter::Node, src: &[u8], results: &mut Vec<FunctionScore>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_item" {
            let name = child
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(src).ok())
                .unwrap_or("")
                .to_string();
            let line = child.start_position().row + 1;
            let impl_type = enclosing_impl_type(child, src);
            let mut contributors = vec![];
            let score = ScoringContext {
                fn_name: &name,
                impl_type: impl_type.as_deref(),
                src,
                contributors: &mut contributors,
            }
            .complexity(child, 0);
            results.push(FunctionScore {
                name,
                line,
                score,
                contributors,
            });
        }
        collect_functions(child, src, results);
    }
}

fn enclosing_impl_type(node: tree_sitter::Node, src: &[u8]) -> Option<String> {
    std::iter::successors(node.parent(), tree_sitter::Node::parent)
        .find(|n| n.kind() == "impl_item")
        .and_then(|n| n.child_by_field_name("type"))
        .and_then(|t| t.utf8_text(src).ok())
        .map(String::from)
}

struct ScoringContext<'a> {
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
        match node.kind() {
            kind @ ("if_expression" | "for_expression" | "while_expression" | "loop_expression"
            | "match_expression") => {
                self.score_flow_break(node, nesting, parse_construct(kind).unwrap())
            }

            "closure_expression" | "function_item" => self.complexity(node, nesting + 1),

            "else_clause" => self.score_else(node, nesting),

            "binary_expression" if is_logical_op(node) => {
                self.score_logical_sequence(node, nesting)
            }

            kind @ ("break_expression" | "continue_expression") if has_label(node) => {
                self.score_jump(node, nesting, parse_jump_keyword(kind).unwrap())
            }

            "call_expression" => {
                if is_recursive_call(node, self.fn_name, self.impl_type, self.src) {
                    self.score_recursion(node, nesting)
                } else {
                    self.complexity(node, nesting)
                }
            }

            _ => self.complexity(node, nesting),
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
        construct: Construct,
    ) -> u64 {
        let increment = 1 + nesting;
        let kind = if nesting > 0 {
            let mut chain = nesting_chain(node);
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
        let is_else_if = node
            .children(&mut node.walk())
            .any(|c| c.kind() == "if_expression");

        if is_else_if {
            self.complexity(node, nesting.saturating_sub(1))
        } else {
            self.push(ContributorKind::Else, node, 1);
            1 + self.complexity(node, nesting)
        }
    }

    fn score_jump(&mut self, node: tree_sitter::Node, nesting: u64, keyword: JumpKeyword) -> u64 {
        let label = label_text(node, self.src).unwrap_or_default().to_string();
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
        collect_logical_operators(node, &mut operators);

        let score = count_operator_sequences(&operators);
        if score > 0 {
            self.push(
                ContributorKind::Logical {
                    operators: operators.iter().map(|&s| s.to_string()).collect(),
                },
                node,
                score,
            );
        }

        score + self.visit_logical_leaves(node, nesting)
    }

    fn visit_logical_leaves(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        let mut cursor = node.walk();
        let children: Vec<_> = node
            .children(&mut cursor)
            .filter(|child| !is_operator_token(*child))
            .collect();

        children
            .into_iter()
            .map(|child| {
                if is_nested_logical(child) {
                    self.visit_logical_leaves(child, nesting)
                } else {
                    self.complexity(child, nesting)
                }
            })
            .sum()
    }
}

fn nesting_chain(node: tree_sitter::Node) -> Vec<Construct> {
    let ancestors = std::iter::successors(node.parent(), tree_sitter::Node::parent);
    let mut chain = Vec::new();
    for parent in ancestors {
        match parent.kind() {
            "function_item" => break,
            "closure_expression" => {
                chain.push(Construct::Closure);
                break;
            }
            kind => chain.extend(parse_construct(kind)),
        }
    }
    chain.reverse();
    chain
}

fn parse_construct(tree_sitter_kind: &str) -> Option<Construct> {
    match tree_sitter_kind {
        "if_expression" => Some(Construct::If),
        "for_expression" => Some(Construct::For),
        "while_expression" => Some(Construct::While),
        "loop_expression" => Some(Construct::Loop),
        "match_expression" => Some(Construct::Match),
        _ => None,
    }
}

fn parse_jump_keyword(tree_sitter_kind: &str) -> Option<JumpKeyword> {
    match tree_sitter_kind {
        "break_expression" => Some(JumpKeyword::Break),
        "continue_expression" => Some(JumpKeyword::Continue),
        _ => None,
    }
}

fn is_logical_op(node: tree_sitter::Node) -> bool {
    node.children(&mut node.walk())
        .any(|c| c.kind() == "&&" || c.kind() == "||")
}

fn logical_operator(node: tree_sitter::Node) -> Option<&'static str> {
    node.children(&mut node.walk())
        .find(|c| c.kind() == "&&" || c.kind() == "||")
        .map(|c| c.kind())
}

fn has_label(node: tree_sitter::Node) -> bool {
    node.children(&mut node.walk()).any(|c| c.kind() == "label")
}

fn label_text<'a>(node: tree_sitter::Node, src: &'a [u8]) -> Option<&'a str> {
    node.children(&mut node.walk())
        .find(|c| c.kind() == "label")
        .and_then(|l| l.utf8_text(src).ok())
}

fn is_recursive_call(
    node: tree_sitter::Node,
    fn_name: &str,
    impl_type: Option<&str>,
    src: &[u8],
) -> bool {
    let Some(target) = node.child_by_field_name("function") else {
        return false;
    };
    callee_name(target, src) == Some(fn_name) && scope_is_self(target, impl_type, src)
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

fn count_operator_sequences(operators: &[&str]) -> u64 {
    operators
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .count() as u64
        + u64::from(!operators.is_empty())
}

fn collect_logical_operators(node: tree_sitter::Node, operators: &mut Vec<&'static str>) {
    if node.kind() != "binary_expression" {
        return;
    }
    let Some(op) = logical_operator(node) else {
        return;
    };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if is_nested_logical(child) {
            collect_logical_operators(child, operators);
        }
    }
    operators.push(op);
}

fn is_nested_logical(node: tree_sitter::Node) -> bool {
    node.kind() == "binary_expression" && is_logical_op(node)
}

fn is_operator_token(node: tree_sitter::Node) -> bool {
    node.kind() == "&&" || node.kind() == "||"
}

#[cfg(test)]
mod tests {
    use super::*;
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
                    construct: Construct::If,
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
                    construct: Construct::If,
                    depth: 1,
                    chain: vec![Construct::For, Construct::If],
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
                    operators: vec!["&&".into(), "||".into()],
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
                    construct: Construct::If,
                    depth: 2,
                    chain: vec![Construct::Closure, Construct::If],
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
}
