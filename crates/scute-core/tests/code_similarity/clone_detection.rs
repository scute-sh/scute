use scute_core::code_similarity::{CloneGroup, SourceTokens, detect_clones, language, tokenize};

const LOW_TOKEN_THRESHOLD: usize = 5;

fn tokenize_rust(source: &str, source_id: &str) -> SourceTokens {
    let tokens = tokenize(source, &language::rust()).unwrap();
    SourceTokens::new(source_id.to_string(), tokens)
}

fn snapshot(groups: &[CloneGroup]) -> String {
    if groups.is_empty() {
        return "(no clones)".to_string();
    }
    groups
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let header = format!("Group {} ({} tokens):", i + 1, g.token_count);
            let occs: Vec<String> = g
                .occurrences
                .iter()
                .map(|o| format!("  {}:{}-{}", o.source_id, o.start_line, o.end_line))
                .collect();
            format!("{header}\n{}", occs.join("\n"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

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
