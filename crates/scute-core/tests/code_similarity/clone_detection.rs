use scute_core::code_similarity::detect_clones;

use super::helpers::{snapshot, tokenize_rust};

const LOW_TOKEN_THRESHOLD: usize = 5;

#[test]
fn partial_overlap_reports_both_groups() {
    let a = tokenize_rust(
        "fn f(x: i32, y: i32) -> i32 { let r = x + y; if r > 0 { return r; } else { return 0; } }",
        "a.rs",
    );
    let b = tokenize_rust(
        "fn g(a: u32, b: u32) -> u32 { let s = a + b; if s > 0 { return s; } else { return 0; } }",
        "b.rs",
    );
    let c = tokenize_rust(
        "fn h(z: f64) -> f64 { let t = z + z; if t > 0 { return t; } else { return 0; } }",
        "c.rs",
    );

    let groups = detect_clones(&[a, b, c], LOW_TOKEN_THRESHOLD);

    insta::assert_snapshot!(snapshot(&groups));
}
