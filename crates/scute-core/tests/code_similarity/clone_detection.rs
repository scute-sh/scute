use scute_core::code_similarity::rust::Rust;

use super::helpers::{parse_and_detect, snapshot};

#[test]
fn partial_overlap_reports_non_subsumed_groups() {
    let result = parse_and_detect(
        &[
            (
                "fn f(x: i32, y: i32) -> i32 { let r = x + y; if r > 0 { return r; } else { return 0; } }",
                "a.rs",
                &Rust,
            ),
            (
                "fn g(a: u32, b: u32) -> u32 { let s = a + b; if s > 0 { return s; } else { return 0; } }",
                "b.rs",
                &Rust,
            ),
            (
                "fn h(z: f64) -> f64 { let t = z + z; if t > 0 { return t; } else { return 0; } }",
                "c.rs",
                &Rust,
            ),
        ],
        5,
    );

    insta::assert_snapshot!(snapshot(&result));
}
