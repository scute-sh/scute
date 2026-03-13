use crate::parser::{AstParser, TreeSitterParser};
use tree_sitter::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Construct {
    If,
    For,
    While,
    Loop,
    Match,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpKeyword {
    Break,
    Continue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContributorKind {
    Structural {
        construct: Construct,
        nesting_depth: u64,
        nesting_chain: Vec<Construct>,
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

pub fn score_functions(source: &str, language: &Language) -> Vec<FunctionScore> {
    let mut parser = TreeSitterParser::new();
    let Ok(tree) = parser.parse(source, language) else {
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
            let score = complexity(
                child,
                0,
                &name,
                impl_type.as_deref(),
                src,
                &mut contributors,
            );
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
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.kind() == "impl_item" {
            return parent
                .child_by_field_name("type")
                .and_then(|t| t.utf8_text(src).ok())
                .map(String::from);
        }
        current = parent;
    }
    None
}

fn complexity(
    node: tree_sitter::Node,
    nesting: u64,
    fn_name: &str,
    impl_type: Option<&str>,
    src: &[u8],
    contributors: &mut Vec<Contributor>,
) -> u64 {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .map(|child| score_node(child, nesting, fn_name, impl_type, src, contributors))
        .sum()
}

fn score_node(
    node: tree_sitter::Node,
    nesting: u64,
    fn_name: &str,
    impl_type: Option<&str>,
    src: &[u8],
    contributors: &mut Vec<Contributor>,
) -> u64 {
    match node.kind() {
        kind @ ("if_expression" | "for_expression" | "while_expression" | "loop_expression"
        | "match_expression") => {
            let construct = parse_construct(kind).unwrap();
            let increment = 1 + nesting;
            let mut chain = nesting_chain(node);
            chain.push(construct);
            contributors.push(Contributor {
                kind: ContributorKind::Structural {
                    construct,
                    nesting_depth: nesting,
                    nesting_chain: chain,
                },
                line: node.start_position().row + 1,
                increment,
            });
            increment + complexity(node, nesting + 1, fn_name, impl_type, src, contributors)
        }

        "closure_expression" | "function_item" => {
            complexity(node, nesting + 1, fn_name, impl_type, src, contributors)
        }

        "else_clause" => score_else(node, nesting, fn_name, impl_type, src, contributors),

        "binary_expression" if is_logical_op(node) => {
            score_logical_sequence(node, nesting, fn_name, impl_type, src, contributors)
        }

        kind @ ("break_expression" | "continue_expression") if has_label(node) => {
            let label = label_text(node, src).unwrap_or_default().to_string();
            let keyword = parse_jump_keyword(kind).unwrap();
            contributors.push(Contributor {
                kind: ContributorKind::Jump { keyword, label },
                line: node.start_position().row + 1,
                increment: 1,
            });
            1 + complexity(node, nesting, fn_name, impl_type, src, contributors)
        }
        "call_expression" if is_recursive_call(node, fn_name, impl_type, src) => {
            contributors.push(Contributor {
                kind: ContributorKind::Recursion {
                    fn_name: fn_name.to_string(),
                },
                line: node.start_position().row + 1,
                increment: 1,
            });
            1 + complexity(node, nesting, fn_name, impl_type, src, contributors)
        }

        _ => complexity(node, nesting, fn_name, impl_type, src, contributors),
    }
}

fn nesting_chain(node: tree_sitter::Node) -> Vec<Construct> {
    let mut chain = vec![];
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.kind() == "function_item" {
            break;
        }
        if let Some(construct) = parse_construct(parent.kind()) {
            chain.push(construct);
        }
        current = parent;
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

fn score_else(
    node: tree_sitter::Node,
    nesting: u64,
    fn_name: &str,
    impl_type: Option<&str>,
    src: &[u8],
    contributors: &mut Vec<Contributor>,
) -> u64 {
    let is_else_if = node
        .children(&mut node.walk())
        .any(|c| c.kind() == "if_expression");

    if is_else_if {
        complexity(
            node,
            nesting.saturating_sub(1),
            fn_name,
            impl_type,
            src,
            contributors,
        )
    } else {
        contributors.push(Contributor {
            kind: ContributorKind::Else,
            line: node.start_position().row + 1,
            increment: 1,
        });
        1 + complexity(node, nesting, fn_name, impl_type, src, contributors)
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

fn score_logical_sequence(
    node: tree_sitter::Node,
    nesting: u64,
    fn_name: &str,
    impl_type: Option<&str>,
    src: &[u8],
    contributors: &mut Vec<Contributor>,
) -> u64 {
    let mut operators = vec![];
    collect_logical_operators(node, &mut operators);

    let score = count_operator_sequences(&operators);
    if score > 0 {
        contributors.push(Contributor {
            kind: ContributorKind::Logical {
                operators: operators.iter().map(|&s| s.to_string()).collect(),
            },
            line: node.start_position().row + 1,
            increment: score,
        });
    }

    score + visit_logical_leaves(node, nesting, fn_name, impl_type, src, contributors)
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

fn visit_logical_leaves(
    node: tree_sitter::Node,
    nesting: u64,
    fn_name: &str,
    impl_type: Option<&str>,
    src: &[u8],
    contributors: &mut Vec<Contributor>,
) -> u64 {
    let mut total = 0;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if is_operator_token(child) {
            continue;
        }
        let scorer = if is_nested_logical(child) {
            visit_logical_leaves
        } else {
            complexity
        };
        total += scorer(child, nesting, fn_name, impl_type, src, contributors);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    fn score_only(source: &str) -> u64 {
        let results = score_functions(source, &tree_sitter_rust::LANGUAGE.into());
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

    #[test]
    fn flat_function_has_no_contributors() {
        let results = score_functions(
            "fn add(a: i32, b: i32) -> i32 { a + b }",
            &tree_sitter_rust::LANGUAGE.into(),
        );

        assert!(results[0].contributors.is_empty());
    }

    #[test]
    fn logical_sequence_appears_as_contributor() {
        let results = score_functions(
            "fn f(a: bool, b: bool, c: bool) -> bool { a && b || c }",
            &tree_sitter_rust::LANGUAGE.into(),
        );

        assert_eq!(
            results[0].contributors,
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
    fn recursion_appears_as_contributor() {
        let results = score_functions(
            "fn factorial(n: u64) -> u64 {
                if n <= 1 { 1 }
                else { n * factorial(n - 1) }
            }",
            &tree_sitter_rust::LANGUAGE.into(),
        );

        let has_recursion = results[0].contributors.iter().any(
            |c| matches!(&c.kind, ContributorKind::Recursion { fn_name } if fn_name == "factorial"),
        );
        assert!(has_recursion);
    }

    #[test]
    fn canonical_example_returns_contributors() {
        use Construct::*;

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
            &tree_sitter_rust::LANGUAGE.into(),
        );

        assert_eq!(
            results[0].contributors,
            vec![
                Contributor {
                    kind: ContributorKind::Structural {
                        construct: For,
                        nesting_depth: 0,
                        nesting_chain: vec![For],
                    },
                    line: 3,
                    increment: 1,
                },
                Contributor {
                    kind: ContributorKind::Structural {
                        construct: If,
                        nesting_depth: 1,
                        nesting_chain: vec![For, If],
                    },
                    line: 4,
                    increment: 2,
                },
                Contributor {
                    kind: ContributorKind::Structural {
                        construct: If,
                        nesting_depth: 2,
                        nesting_chain: vec![For, If, If],
                    },
                    line: 5,
                    increment: 3,
                },
                Contributor {
                    kind: ContributorKind::Else,
                    line: 6,
                    increment: 1,
                },
            ]
        );
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
            &tree_sitter_rust::LANGUAGE.into(),
        );
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "outer");
        assert_eq!(results[0].score, 3);
        assert_eq!(results[1].name, "inner");
        assert_eq!(results[1].score, 1);
    }
}
