use super::rules::LanguageRules;
use super::rust::Rust;
use super::score::score_functions;
use super::typescript::TypeScript;
use test_case::test_case;

fn ts() -> TypeScript {
    TypeScript::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
}

fn expect_score(source: &str, rules: &dyn LanguageRules, expected: u64) {
    let results = score_functions(source, rules);
    assert_eq!(results.len(), 1, "expected exactly one function");
    assert_eq!(results[0].score, expected);
}

// --- flat function ---

#[test_case(&Rust, "fn f(a: i32, b: i32) -> i32 { a + b }" ; "rust")]
#[test_case(&ts(), "function f(a: number, b: number) { return a + b }" ; "typescript")]
fn flat_function_scores_zero(rules: &dyn LanguageRules, source: &str) {
    expect_score(source, rules, 0);
}

// --- structural increments: conditionals ---

#[test_case(&Rust, "fn f(x: i32) { if x > 0 { return; } }" ; "rust")]
#[test_case(&ts(), "function f(x: number) { if (x > 0) { return; } }" ; "typescript")]
fn scores_if(rules: &dyn LanguageRules, source: &str) {
    expect_score(source, rules, 1);
}

#[test_case(&Rust, "fn f(x: i32) -> i32 { match x { 0 => 1, _ => 2 } }" ; "rust_match")]
#[test_case(&ts(), "function f(x: number) { switch (x) { case 1: break; } }" ; "typescript_switch")]
fn scores_branch(rules: &dyn LanguageRules, source: &str) {
    expect_score(source, rules, 1);
}

#[test]
fn scores_ternary() {
    expect_score("function f(x: boolean) { return x ? 1 : 0; }", &ts(), 1);
}

// --- structural increments: loops ---

#[test_case(&Rust, "fn f(items: &[i32]) { for _ in items {} }" ; "rust_for")]
#[test_case(&Rust, "fn f() { while true {} }" ; "rust_while")]
#[test_case(&Rust, "fn f() { loop {} }" ; "rust_loop")]
#[test_case(&ts(), "function f() { for (let i = 0; i < 10; i++) {} }" ; "typescript_for")]
#[test_case(&ts(), "function f(obj: any) { for (const k in obj) {} }" ; "typescript_for_in")]
#[test_case(&ts(), "function f(items: number[]) { for (const x of items) {} }" ; "typescript_for_of")]
#[test_case(&ts(), "function f(x: number) { while (x > 0) { x--; } }" ; "typescript_while")]
#[test_case(&ts(), "function f(x: number) { do { x--; } while (x > 0); }" ; "typescript_do_while")]
fn scores_loop(rules: &dyn LanguageRules, source: &str) {
    expect_score(source, rules, 1);
}

#[test]
fn scores_catch() {
    expect_score("function f() { try {} catch (e) {} }", &ts(), 1);
}

// --- hybrid increments: else ---

#[test_case(&Rust, "fn f(x: bool) { if x {} else {} }" ; "rust")]
#[test_case(&ts(), "function f(x: number) { if (x > 0) { return 1; } else { return -1; } }" ; "typescript")]
fn scores_else(rules: &dyn LanguageRules, source: &str) {
    expect_score(source, rules, 2);
}

// if: +1, else if: +1 (flat), else: +1
#[test_case(&Rust, "fn f(x: i32) -> i32 {
    if x > 0 { 1 }
    else if x < 0 { -1 }
    else { 0 }
}" ; "rust")]
#[test_case(&ts(), "function f(x: number) {
    if (x > 0) { return 1; }
    else if (x < 0) { return -1; }
    else { return 0; }
}" ; "typescript")]
fn scores_else_if_chain_flat(rules: &dyn LanguageRules, source: &str) {
    expect_score(source, rules, 3);
}

// --- logical operator sequences ---

#[test_case(&Rust, "fn f(a: bool, b: bool, c: bool) -> bool { a && b && c }", 1 ; "rust_same_ops")]
#[test_case(&Rust, "fn f(a: bool, b: bool, c: bool) -> bool { a && b || c }", 2 ; "rust_mixed_ops")]
#[test_case(&ts(), "function f(a: boolean, b: boolean, c: boolean) { return a && b && c; }", 1 ; "typescript_same_ops")]
#[test_case(&ts(), "function f(a: boolean, b: boolean, c: boolean) { return a && b || c; }", 2 ; "typescript_mixed_ops")]
fn scores_logical_operators(rules: &dyn LanguageRules, source: &str, expected: u64) {
    expect_score(source, rules, expected);
}

#[test]
fn ignores_nullish_coalescing() {
    expect_score("function f(a: any, b: any) { return a ?? b; }", &ts(), 0);
}

// --- recursion ---

// if: +1, else: +1, recursion: +1
#[test_case(&Rust, "fn factorial(n: u64) -> u64 {
    if n <= 1 { 1 }
    else { n * factorial(n - 1) }
}" ; "rust")]
#[test_case(&ts(), "function factorial(n: number): number {
    if (n <= 1) { return 1; }
    else { return n * factorial(n - 1); }
}" ; "typescript")]
fn scores_direct_recursion(rules: &dyn LanguageRules, source: &str) {
    expect_score(source, rules, 3);
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
    expect_score(source, &Rust, expected);
}

// --- labeled jumps ---

// outer loop: +1, inner loop: +2, if: +3, labeled break: +1
#[test_case(&Rust, "fn f(items: &[&[i32]]) -> i32 {
    let mut total = 0;
    'outer: for row in items {
        for item in *row {
            if *item < 0 { break 'outer; }
            total += item;
        }
    }
    total
}" ; "rust")]
#[test_case(&ts(), "function f(matrix: number[][]) {
    let total = 0;
    outer: for (const row of matrix) {
        for (const item of row) {
            if (item < 0) { break outer; }
            total += item;
        }
    }
    return total;
}" ; "typescript")]
fn scores_labeled_break(rules: &dyn LanguageRules, source: &str) {
    expect_score(source, rules, 7);
}

// --- nesting: inline boundaries (closures / arrow functions) ---

// closure/arrow: nesting +1, if: +1+1, else: +1
#[test_case(&Rust, "fn f(items: &[i32]) -> Vec<i32> {
    items.iter().filter(|x| {
        if **x > 0 { true } else { false }
    }).copied().collect()
}" ; "rust_closure")]
#[test_case(&ts(), "function f(items: number[]) {
    return items.filter((x) => {
        if (x > 0) { return true; }
        else { return false; }
    });
}" ; "typescript_arrow")]
fn scores_inline_nesting(rules: &dyn LanguageRules, source: &str) {
    expect_score(source, rules, 3);
}

// --- nesting: separate scoring units ---

#[test_case(&Rust,
    "fn outer() { fn inner() { if true {} } if true {} }",
    "outer", 3, "inner", 1
    ; "rust"
)]
#[test_case(&ts(),
    "function outer() { function inner() { if (true) {} } if (true) {} }",
    "outer", 3, "inner", 1
    ; "typescript"
)]
fn scores_nested_function_independently(
    rules: &dyn LanguageRules,
    source: &str,
    outer_name: &str,
    outer_score: u64,
    inner_name: &str,
    inner_score: u64,
) {
    let results = score_functions(source, rules);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, outer_name);
    assert_eq!(results[0].score, outer_score);
    assert_eq!(results[1].name, inner_name);
    assert_eq!(results[1].score, inner_score);
}

// --- scoring unit discovery ---

#[test_case(&Rust, "struct S;
impl S {
    fn method(&self, x: i32) -> i32 {
        if x > 0 { 1 } else { -1 }
    }
}", 2 ; "rust_impl_method")]
#[test_case(&ts(), "class Calc {
    check(x: number) { if (x > 0) { return true; } return false; }
}", 1 ; "typescript_class_method")]
fn scores_method(rules: &dyn LanguageRules, source: &str, expected: u64) {
    expect_score(source, rules, expected);
}
