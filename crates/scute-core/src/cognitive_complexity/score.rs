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
    let mut cursor = tree.root_node().walk();
    for node in tree.root_node().children(&mut cursor) {
        if node.kind() == "function_item" {
            let name = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(src).ok())
                .unwrap_or("")
                .to_string();
            let line = node.start_position().row + 1;
            let score = complexity(node, 0, &name, src);
            results.push(FunctionScore { name, line, score });
        }
    }
    results
}

fn complexity(node: tree_sitter::Node, nesting: u64, fn_name: &str, src: &[u8]) -> u64 {
    let mut total = 0;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "if_expression" | "for_expression" | "while_expression" | "loop_expression"
            | "match_expression" => {
                total += 1 + nesting;
                total += complexity(child, nesting + 1, fn_name, src);
            }
            "closure_expression" | "function_item" => {
                total += complexity(child, nesting + 1, fn_name, src);
            }
            "else_clause" => {
                let has_if = child
                    .children(&mut child.walk())
                    .any(|c| c.kind() == "if_expression");
                if has_if {
                    total += complexity(child, nesting - 1, fn_name, src);
                } else {
                    total += 1;
                    total += complexity(child, nesting, fn_name, src);
                }
            }
            "binary_expression" if is_logical_op(child) => {
                total += score_logical_sequence(child, nesting, fn_name, src);
            }
            "break_expression" | "continue_expression" if has_label(child) => {
                total += 1;
                total += complexity(child, nesting, fn_name, src);
            }
            "call_expression" if is_recursive_call(child, fn_name, src) => {
                total += 1;
                total += complexity(child, nesting, fn_name, src);
            }
            _ => {
                total += complexity(child, nesting, fn_name, src);
            }
        }
    }

    total
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

fn is_recursive_call(node: tree_sitter::Node, fn_name: &str, src: &[u8]) -> bool {
    node.child_by_field_name("function")
        .and_then(|f| f.utf8_text(src).ok())
        .is_some_and(|name| name == fn_name)
}

fn score_logical_sequence(node: tree_sitter::Node, nesting: u64, fn_name: &str, src: &[u8]) -> u64 {
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

    score + visit_logical_leaves(node, nesting, fn_name, src)
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

fn visit_logical_leaves(node: tree_sitter::Node, nesting: u64, fn_name: &str, src: &[u8]) -> u64 {
    let mut total = 0;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "binary_expression" && is_logical_op(child) {
            total += visit_logical_leaves(child, nesting, fn_name, src);
        } else if child.kind() != "&&" && child.kind() != "||" {
            total += complexity(child, nesting, fn_name, src);
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

    #[test_case("fn add(a: i32, b: i32) -> i32 { a + b }", 0 ; "flat function")]
    #[test_case("fn f(x: i32) -> bool { if x > 0 { return true; } false }", 1 ; "single if")]
    #[test_case("fn f(a: bool, b: bool, c: bool) -> bool { a && b && c }", 1 ; "same logical operators")]
    #[test_case("fn f(a: bool, b: bool, c: bool) -> bool { a && b || c }", 2 ; "mixed logical operators")]
    // if: +1, else if: +1 (flat chain), else: +1
    #[test_case("fn f(x: i32) -> i32 {
        if x > 0 { 1 }
        else if x < 0 { -1 }
        else { 0 }
    }", 3 ; "else if chain scores flat")]
    // if: +1, else: +1, recursion: +1
    #[test_case("fn factorial(n: u64) -> u64 {
        if n <= 1 { 1 }
        else { n * factorial(n - 1) }
    }", 3 ; "direct recursion adds 1")]
    // closure: +0 structural, nesting becomes 1; if: +1+1, else: +1
    #[test_case("fn f(items: &[i32]) -> Vec<i32> {
        items.iter().filter(|x| {
            if **x > 0 { true } else { false }
        }).copied().collect()
    }", 3 ; "closure increases nesting")]
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
    }", 7 ; "canonical example")]
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
    }", 7 ; "labeled break adds 1")]
    fn scores(source: &str, expected: u64) {
        assert_eq!(score_only(source), expected);
    }

    // inner fn: +0 structural, nesting becomes 1
    // if inside inner: +1 +1(nesting) = 2, if in outer: +1 = total 3
    // inner should NOT appear as separate scored function
    #[test]
    fn nested_function_increases_nesting() {
        let results = score_functions(
            "fn outer() {
                fn inner() { if true {} }
                if true {}
            }",
            &tree_sitter_rust::LANGUAGE.into(),
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "outer");
        assert_eq!(results[0].score, 3);
    }
}
