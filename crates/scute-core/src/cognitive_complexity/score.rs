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

    fn score_rust(source: &str) -> Vec<(String, u64)> {
        score_functions(source, &tree_sitter_rust::LANGUAGE.into())
            .into_iter()
            .map(|f| (f.name, f.score))
            .collect()
    }

    #[test]
    fn flat_function_scores_0() {
        let source = "fn add(a: i32, b: i32) -> i32 { a + b }";

        let results = score_rust(source);

        assert_eq!(results, vec![("add".to_string(), 0)]);
    }

    #[test]
    fn single_if_scores_1() {
        let source = "fn check(x: i32) -> bool { if x > 0 { return true; } false }";

        let results = score_rust(source);

        assert_eq!(results, vec![("check".to_string(), 1)]);
    }

    #[test]
    fn canonical_example_scores_7() {
        let source = r"
fn process(items: &[i32]) -> i32 {
    let mut total = 0;
    for item in items {
        if *item > 0 {
            if *item > 10 {
                total += item;
            } else {
                total -= item;
            }
        }
    }
    total
}
";

        let results = score_rust(source);

        assert_eq!(results, vec![("process".to_string(), 7)]);
    }

    #[test]
    fn else_if_chain_scores_flat() {
        let source = r"
fn f(x: i32) -> i32 {
    if x > 0 {
        1
    } else if x < 0 {
        -1
    } else {
        0
    }
}
";

        let results = score_rust(source);

        // if: +1 (nesting 0), else if: +1 (nesting 0, flat chain), else: +1 (hybrid) = 3
        assert_eq!(results, vec![("f".to_string(), 3)]);
    }

    #[test]
    fn same_logical_operators_score_1() {
        let source = "fn f(a: bool, b: bool, c: bool) -> bool { a && b && c }";

        let results = score_rust(source);

        assert_eq!(results, vec![("f".to_string(), 1)]);
    }

    #[test]
    fn mixed_logical_operators_score_per_change() {
        let source = "fn f(a: bool, b: bool, c: bool) -> bool { a && b || c }";

        let results = score_rust(source);

        assert_eq!(results, vec![("f".to_string(), 2)]);
    }

    #[test]
    fn direct_recursion_adds_1() {
        let source = r"
fn factorial(n: u64) -> u64 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}
";

        let results = score_rust(source);

        // if: +1, else: +1, recursion: +1 = 3
        assert_eq!(results, vec![("factorial".to_string(), 3)]);
    }

    #[test]
    fn closure_increases_nesting_without_structural_increment() {
        let source = r"
fn f(items: &[i32]) -> Vec<i32> {
    items.iter().filter(|x| {
        if **x > 0 {
            true
        } else {
            false
        }
    }).copied().collect()
}
";

        let results = score_rust(source);

        // closure: +0 (no structural increment, but nesting becomes 1)
        // if inside closure: +1 (structural) + 1 (nesting) = +2
        // else: +1 (hybrid)
        // total = 0 + 2 + 1 = 3
        assert_eq!(results, vec![("f".to_string(), 3)]);
    }

    #[test]
    fn labeled_break_adds_1() {
        let source = r"
fn f(items: &[&[i32]]) -> i32 {
    let mut total = 0;
    'outer: for row in items {
        for item in *row {
            if *item < 0 {
                break 'outer;
            }
            total += item;
        }
    }
    total
}
";

        let results = score_rust(source);

        // outer for: +1 (nesting 0)
        // inner for: +1 +1 (nesting 1) = +2
        // if: +1 +2 (nesting 2) = +3
        // break 'outer: +1 (fundamental, no nesting)
        // total = 1 + 2 + 3 + 1 = 7
        assert_eq!(results, vec![("f".to_string(), 7)]);
    }

    #[test]
    fn nested_function_increases_nesting_without_structural_increment() {
        let source = r"
fn outer() {
    fn inner() {
        if true {}
    }
    if true {}
}
";

        let results = score_rust(source);

        // inner fn: +0 (no structural increment, but nesting becomes 1)
        // if inside inner: +1 +1 (nesting 1) = +2
        // if in outer: +1 (nesting 0)
        // total for outer = 0 + 2 + 1 = 3
        // inner should NOT appear as a separate scored function
        assert_eq!(results, vec![("outer".to_string(), 3)]);
    }
}
