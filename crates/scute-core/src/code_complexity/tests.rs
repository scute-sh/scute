use crate::code_complexity::check::languages;
use crate::files::SourceFile;

use super::score::score_functions;
use test_case::test_case;

fn score(source: &str, path: &str) -> Vec<super::score::FunctionScore> {
    let file = SourceFile {
        path: path.into(),
        content: source.into(),
    };
    score_functions(&file, &languages())
}

fn expect_score(source: &str, path: &str, expected: u64) {
    let results = score(source, path);
    assert_eq!(results.len(), 1, "expected exactly one function");
    assert_eq!(results[0].score, expected);
}

fn assert_function_score(results: &[super::score::FunctionScore], name: &str, expected: u64) {
    let func = results
        .iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no function named '{name}'"));
    assert_eq!(func.score, expected, "wrong score for '{name}'");
}

#[test_case("a.rs", "fn f(a: i32, b: i32) -> i32 { a + b }" ; "rust")]
#[test_case("a.ts", "function f(a: number, b: number) { return a + b }" ; "typescript")]
fn flat_function_scores_zero(path: &str, source: &str) {
    expect_score(source, path, 0);
}

#[test_case("a.rs", "fn f(x: i32) { if x > 0 { return; } }" ; "rust")]
#[test_case("a.ts", "function f(x: number) { if (x > 0) { return; } }" ; "typescript")]
fn scores_if(path: &str, source: &str) {
    expect_score(source, path, 1);
}

#[test_case("a.rs", "fn f(x: i32) -> i32 { match x { 0 => 1, _ => 2 } }" ; "rust_match")]
#[test_case("a.ts", "function f(x: number) { switch (x) { case 1: break; } }" ; "typescript_switch")]
fn scores_branch(path: &str, source: &str) {
    expect_score(source, path, 1);
}

#[test]
fn scores_ternary() {
    expect_score("function f(x: boolean) { return x ? 1 : 0; }", "a.ts", 1);
}

// if: +1, nested ternary: +1+1 (nesting=1)
#[test]
fn scores_nested_ternary_with_nesting_penalty() {
    expect_score(
        "function f(x: number) { if (x > 0) { return x > 10 ? 1 : 0; } }",
        "a.ts",
        3,
    );
}

#[test_case("a.rs", "fn f(items: &[i32]) { for _ in items {} }" ; "rust_for")]
#[test_case("a.rs", "fn f() { while true {} }" ; "rust_while")]
#[test_case("a.rs", "fn f() { loop {} }" ; "rust_loop")]
#[test_case("a.ts", "function f() { for (let i = 0; i < 10; i++) {} }" ; "typescript_for")]
#[test_case("a.ts", "function f(obj: any) { for (const k in obj) {} }" ; "typescript_for_in")]
#[test_case("a.ts", "function f(items: number[]) { for (const x of items) {} }" ; "typescript_for_of")]
#[test_case("a.ts", "function f(x: number) { while (x > 0) { x--; } }" ; "typescript_while")]
#[test_case("a.ts", "function f(x: number) { do { x--; } while (x > 0); }" ; "typescript_do_while")]
fn scores_loop(path: &str, source: &str) {
    expect_score(source, path, 1);
}

#[test]
fn scores_catch() {
    expect_score("function f() { try {} catch (e) {} }", "a.ts", 1);
}

#[test_case("a.rs", "fn f(x: bool) { if x {} else {} }" ; "rust")]
#[test_case("a.ts", "function f(x: number) { if (x > 0) { return 1; } else { return -1; } }" ; "typescript")]
fn scores_else(path: &str, source: &str) {
    expect_score(source, path, 2);
}

// if: +1, else if: +1 (flat), else: +1
#[test_case("a.rs", "fn f(x: i32) -> i32 {
    if x > 0 { 1 }
    else if x < 0 { -1 }
    else { 0 }
}" ; "rust")]
#[test_case("a.ts", "function f(x: number) {
    if (x > 0) { return 1; }
    else if (x < 0) { return -1; }
    else { return 0; }
}" ; "typescript")]
fn scores_else_if_chain_flat(path: &str, source: &str) {
    expect_score(source, path, 3);
}

#[test_case("a.rs", "fn f(a: bool, b: bool, c: bool) -> bool { a && b && c }", 1 ; "rust_same_ops")]
#[test_case("a.rs", "fn f(a: bool, b: bool, c: bool) -> bool { a && b || c }", 2 ; "rust_mixed_ops")]
#[test_case("a.ts", "function f(a: boolean, b: boolean, c: boolean) { return a && b && c; }", 1 ; "typescript_same_ops")]
#[test_case("a.ts", "function f(a: boolean, b: boolean, c: boolean) { return a && b || c; }", 2 ; "typescript_mixed_ops")]
fn scores_logical_operators(path: &str, source: &str, expected: u64) {
    expect_score(source, path, expected);
}

#[test]
fn ignores_nullish_coalescing() {
    expect_score("function f(a: any, b: any) { return a ?? b; }", "a.ts", 0);
}

// if: +1, else: +1, recursion: +1
#[test_case("a.rs", "fn factorial(n: u64) -> u64 {
    if n <= 1 { 1 }
    else { n * factorial(n - 1) }
}" ; "rust")]
#[test_case("a.ts", "function factorial(n: number): number {
    if (n <= 1) { return 1; }
    else { return n * factorial(n - 1); }
}" ; "typescript")]
fn scores_direct_recursion(path: &str, source: &str) {
    expect_score(source, path, 3);
}

// if: +1, else: +1, this.method() recursion: +1
#[test]
fn scores_this_method_recursion() {
    expect_score(
        "class C {
    count(n: number): number {
        if (n <= 1) { return 1; }
        else { return n * this.count(n - 1); }
    }
}",
        "a.ts",
        3,
    );
}

// Rust-specific: self.method() and Self::method() recursion
#[test_case("struct S;
impl S {
    fn count(&self, n: u64) -> u64 {
        if n <= 1 { 1 }
        else { n * self.count(n - 1) }
    }
}", 3 ; "self_method")]
#[test_case("struct S;
impl S {
    fn count(n: u64) -> u64 {
        if n <= 1 { 1 }
        else { n * Self::count(n - 1) }
    }
}", 3 ; "associated_function")]
#[test_case("struct Abc;
struct Def;
impl Abc {
    fn foo(n: u64) -> u64 {
        if n <= 1 { 1 }
        else { Def::foo(n - 1) }
    }
}", 2 ; "different_type_is_not_recursion")]
fn scores_rust_qualified_recursion(source: &str, expected: u64) {
    expect_score(source, "a.rs", expected);
}

// outer loop: +1, inner loop: +2, if: +3, labeled break: +1
#[test_case("a.rs", "fn f(items: &[&[i32]]) -> i32 {
    let mut total = 0;
    'outer: for row in items {
        for item in *row {
            if *item < 0 { break 'outer; }
            total += item;
        }
    }
    total
}" ; "rust")]
#[test_case("a.ts", "function f(matrix: number[][]) {
    let total = 0;
    outer: for (const row of matrix) {
        for (const item of row) {
            if (item < 0) { break outer; }
            total += item;
        }
    }
    return total;
}" ; "typescript")]
fn scores_labeled_break(path: &str, source: &str) {
    expect_score(source, path, 7);
}

// closure/arrow: nesting +1, if: +1+1, else: +1
#[test_case("a.rs", "fn f(items: &[i32]) -> Vec<i32> {
    items.iter().filter(|x| {
        if **x > 0 { true } else { false }
    }).copied().collect()
}" ; "rust_closure")]
#[test_case("a.ts", "function f(items: number[]) {
    return items.filter((x) => {
        if (x > 0) { return true; }
        else { return false; }
    });
}" ; "typescript_arrow")]
fn scores_inline_nesting(path: &str, source: &str) {
    expect_score(source, path, 3);
}

// function expression: const f = function() {...}
// nesting +1, if: +1+1, else: +1
#[test]
fn scores_function_expression_as_inline_nesting() {
    expect_score(
        "function f() {
    const g = function() {
        if (true) { return 1; }
        else { return 0; }
    };
}",
        "a.ts",
        3,
    );
}

// generator declaration: behaves like nested named function (Separate)
#[test]
fn scores_generator_declaration_independently() {
    let results = score(
        "function outer() { function* gen() { if (true) {} } if (true) {} }",
        "a.ts",
    );
    assert_function_score(&results, "outer", 3);
    assert_function_score(&results, "gen", 1);
}

#[test_case("a.rs",
    "fn outer() { fn inner() { if true {} } if true {} }",
    "outer", 3, "inner", 1
    ; "rust"
)]
#[test_case("a.ts",
    "function outer() { function inner() { if (true) {} } if (true) {} }",
    "outer", 3, "inner", 1
    ; "typescript"
)]
fn scores_nested_function_independently(
    path: &str,
    source: &str,
    outer_name: &str,
    outer_score: u64,
    inner_name: &str,
    inner_score: u64,
) {
    let results = score(source, path);
    assert_function_score(&results, outer_name, outer_score);
    assert_function_score(&results, inner_name, inner_score);
}

#[test_case("a.rs", "struct S;
impl S {
    fn method(&self, x: i32) -> i32 {
        if x > 0 { 1 } else { -1 }
    }
}", 2 ; "rust_impl_method")]
#[test_case("a.ts", "class Calc {
    check(x: number) { if (x > 0) { return true; } return false; }
}", 1 ; "typescript_class_method")]
fn scores_method(path: &str, source: &str, expected: u64) {
    expect_score(source, path, expected);
}

#[test_case("a.rs", "trait Service { fn process(&self, input: &str) -> bool; fn save(&self, data: &str); }" ; "rust_trait")]
#[test_case("a.ts", "interface Service { process(input: string): boolean; save(data: any): void; }" ; "typescript_interface")]
fn type_declarations_with_method_signatures_return_no_functions(path: &str, source: &str) {
    let results = score(source, path);
    assert!(results.is_empty());
}

// if: +1, else: +1. other.count() is NOT recursion.
#[test]
fn ignores_non_this_member_call_as_recursion() {
    expect_score(
        "class C {
            count(n: number): number {
                if (n <= 1) { return 1; }
                else { return n * other.count(n - 1); }
            }
        }",
        "a.ts",
        2,
    );
}

// JS grammar smoke test: exercises all construct families through the JS parser
// to verify grammar wiring. Scoring logic is shared with TS and tested above.
#[test]
fn javascript_grammar_scores_all_constructs() {
    // for...of: +1, if: +1+1(nesting), nested if: +1+2(nesting),
    // &&: +1, break outer: +1, else: +1, recursion: +1 = 10
    expect_score(
        "function process(items) {
            outer: for (const item of items) {
                if (item > 0) {
                    if (item > 10 && item < 100) {
                        break outer;
                    }
                } else {
                    return process(items.slice(1));
                }
            }
        }",
        "a.js",
        10,
    );
}
