use crate::parser::{AstParser, TreeSitterParser};
use tree_sitter::Language;

pub struct Contributor {
    pub kind: String,
    pub line: usize,
    pub increment: u64,
    pub nesting_depth: u64,
    pub nesting_chain: Vec<String>,
    pub detail: Option<String>,
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
        "if_expression" | "for_expression" | "while_expression" | "loop_expression"
        | "match_expression" => {
            let increment = 1 + nesting;
            let mut chain = nesting_chain(node);
            chain.push(structural_kind(node.kind()).to_string());
            contributors.push(Contributor {
                kind: structural_kind(node.kind()).into(),
                line: node.start_position().row + 1,
                increment,
                nesting_depth: nesting,
                nesting_chain: chain,
                detail: None,
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

        "break_expression" | "continue_expression" if has_label(node) => {
            let label = label_text(node, src).unwrap_or_default().to_string();
            contributors.push(Contributor {
                kind: labeled_kind(node.kind()).into(),
                line: node.start_position().row + 1,
                increment: 1,
                nesting_depth: nesting,
                nesting_chain: vec![],
                detail: Some(label),
            });
            1 + complexity(node, nesting, fn_name, impl_type, src, contributors)
        }
        "call_expression" if is_recursive_call(node, fn_name, impl_type, src) => {
            contributors.push(Contributor {
                kind: "recursion".into(),
                line: node.start_position().row + 1,
                increment: 1,
                nesting_depth: nesting,
                nesting_chain: vec![],
                detail: Some(fn_name.to_string()),
            });
            1 + complexity(node, nesting, fn_name, impl_type, src, contributors)
        }

        _ => complexity(node, nesting, fn_name, impl_type, src, contributors),
    }
}

fn nesting_chain(node: tree_sitter::Node) -> Vec<String> {
    let mut chain = vec![];
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.kind() == "function_item" {
            break;
        }
        if let Some(name) = structural_kind_opt(parent.kind()) {
            chain.push(name.to_string());
        }
        current = parent;
    }
    chain.reverse();
    chain
}

fn structural_kind_opt(tree_sitter_kind: &str) -> Option<&str> {
    match tree_sitter_kind {
        "if_expression" => Some("if"),
        "for_expression" => Some("for"),
        "while_expression" => Some("while"),
        "loop_expression" => Some("loop"),
        "match_expression" => Some("match"),
        "else_clause" => Some("else"),
        _ => None,
    }
}

fn structural_kind(tree_sitter_kind: &str) -> &str {
    structural_kind_opt(tree_sitter_kind).unwrap_or(tree_sitter_kind)
}

fn labeled_kind(tree_sitter_kind: &str) -> &str {
    match tree_sitter_kind {
        "break_expression" => "break",
        "continue_expression" => "continue",
        _ => tree_sitter_kind,
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
            kind: "else".into(),
            line: node.start_position().row + 1,
            increment: 1,
            nesting_depth: nesting,
            nesting_chain: vec![],
            detail: None,
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

fn format_ops_detail(operators: &[&str]) -> String {
    let mut seen: Vec<&str> = vec![];
    for op in operators {
        if !seen.contains(op) {
            seen.push(op);
        }
    }
    seen.iter()
        .map(|o| format!("'{o}'"))
        .collect::<Vec<_>>()
        .join(" and ")
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
            kind: "logical".into(),
            line: node.start_position().row + 1,
            increment: score,
            nesting_depth: nesting,
            nesting_chain: vec![],
            detail: Some(format_ops_detail(&operators)),
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

        let contributors = &results[0].contributors;
        assert_eq!(contributors.len(), 1);
        assert_eq!(contributors[0].kind, "logical");
        assert_eq!(contributors[0].increment, 2);
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

        let kinds: Vec<&str> = results[0]
            .contributors
            .iter()
            .map(|c| c.kind.as_str())
            .collect();
        assert!(kinds.contains(&"recursion"));
    }

    #[test]
    fn canonical_example_returns_contributors() {
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

        let contributors = &results[0].contributors;
        assert_eq!(contributors.len(), 4);

        assert_eq!(contributors[0].kind, "for");
        assert_eq!(contributors[0].increment, 1);
        assert_eq!(contributors[0].nesting_depth, 0);
        assert_eq!(contributors[0].nesting_chain, vec!["for"]);

        assert_eq!(contributors[1].kind, "if");
        assert_eq!(contributors[1].increment, 2);
        assert_eq!(contributors[1].nesting_depth, 1);
        assert_eq!(contributors[1].nesting_chain, vec!["for", "if"]);

        assert_eq!(contributors[2].kind, "if");
        assert_eq!(contributors[2].increment, 3);
        assert_eq!(contributors[2].nesting_depth, 2);
        assert_eq!(contributors[2].nesting_chain, vec!["for", "if", "if"]);

        assert_eq!(contributors[3].kind, "else");
        assert_eq!(contributors[3].increment, 1);
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
