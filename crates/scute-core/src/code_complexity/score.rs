use crate::parser::{AstParser, TreeSitterParser};
use tree_sitter::Language;

pub struct FunctionScore {
    pub name: String,
    pub line: usize,
    pub score: u64,
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
            let score = complexity(child, 0, &name, impl_type.as_deref(), src);
            results.push(FunctionScore { name, line, score });
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
) -> u64 {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .map(|child| score_node(child, nesting, fn_name, impl_type, src))
        .sum()
}

fn score_node(
    node: tree_sitter::Node,
    nesting: u64,
    fn_name: &str,
    impl_type: Option<&str>,
    src: &[u8],
) -> u64 {
    match node.kind() {
        // structural: +1 with nesting penalty, increase depth
        "if_expression" | "for_expression" | "while_expression" | "loop_expression"
        | "match_expression" => {
            1 + nesting + complexity(node, nesting + 1, fn_name, impl_type, src)
        }

        // nesting only: closures and nested functions increase depth
        "closure_expression" | "function_item" => {
            complexity(node, nesting + 1, fn_name, impl_type, src)
        }

        // else: plain else +1, else-if chains stay flat
        "else_clause" => score_else(node, nesting, fn_name, impl_type, src),

        // logical sequences: +1 per operator change
        "binary_expression" if is_logical_op(node) => {
            score_logical_sequence(node, nesting, fn_name, impl_type, src)
        }

        // labeled jumps and recursion: flat +1
        "break_expression" | "continue_expression" if has_label(node) => {
            1 + complexity(node, nesting, fn_name, impl_type, src)
        }
        "call_expression" if is_recursive_call(node, fn_name, impl_type, src) => {
            1 + complexity(node, nesting, fn_name, impl_type, src)
        }

        _ => complexity(node, nesting, fn_name, impl_type, src),
    }
}

fn score_else(
    node: tree_sitter::Node,
    nesting: u64,
    fn_name: &str,
    impl_type: Option<&str>,
    src: &[u8],
) -> u64 {
    let is_else_if = node
        .children(&mut node.walk())
        .any(|c| c.kind() == "if_expression");

    if is_else_if {
        // else-if chains are flat — undo the nesting bump from the parent if
        complexity(node, nesting.saturating_sub(1), fn_name, impl_type, src)
    } else {
        1 + complexity(node, nesting, fn_name, impl_type, src)
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

fn score_logical_sequence(
    node: tree_sitter::Node,
    nesting: u64,
    fn_name: &str,
    impl_type: Option<&str>,
    src: &[u8],
) -> u64 {
    let mut operators = vec![];
    collect_logical_operators(node, &mut operators);

    let mut score = 0;
    let mut last_op: Option<&str> = None;
    for op in &operators {
        if last_op != Some(op) {
            score += 1;
        }
        last_op = Some(op);
    }

    score + visit_logical_leaves(node, nesting, fn_name, impl_type, src)
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
        if child.kind() == "binary_expression" && is_logical_op(child) {
            collect_logical_operators(child, operators);
        }
    }
    operators.push(op);
}

fn visit_logical_leaves(
    node: tree_sitter::Node,
    nesting: u64,
    fn_name: &str,
    impl_type: Option<&str>,
    src: &[u8],
) -> u64 {
    let mut total = 0;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "binary_expression" && is_logical_op(child) {
            total += visit_logical_leaves(child, nesting, fn_name, impl_type, src);
        } else if child.kind() != "&&" && child.kind() != "||" {
            total += complexity(child, nesting, fn_name, impl_type, src);
        }
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
