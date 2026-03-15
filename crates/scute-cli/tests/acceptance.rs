mod discovery {
    use scute_test_utils::{Interface, Scute};
    use test_case::test_case;

    use Interface::{Cli, Mcp};

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn lists_commit_message_check(interface: Interface) {
        Scute::new(interface)
            .list_checks()
            .expect_contains("commit-message");
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn lists_dependency_freshness_check(interface: Interface) {
        Scute::new(interface)
            .list_checks()
            .expect_contains("dependency-freshness");
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn lists_code_similarity_check(interface: Interface) {
        Scute::new(interface)
            .list_checks()
            .expect_contains("code-similarity");
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn lists_code_complexity_check(interface: Interface) {
        Scute::new(interface)
            .list_checks()
            .expect_contains("code-complexity");
    }
}

mod commit_message {
    use scute_test_utils::{Interface, Scute};
    use test_case::test_case;

    use Interface::{Cli, CliStdin, Mcp};

    #[test_case(Cli)]
    #[test_case(CliStdin)]
    #[test_case(Mcp)]
    fn valid_message_passes(interface: Interface) {
        Scute::new(interface)
            .check(&["commit-message", "feat: add login"])
            .expect_pass();
    }

    #[test_case(Cli)]
    #[test_case(CliStdin)]
    #[test_case(Mcp)]
    fn invalid_message_fails(interface: Interface) {
        Scute::new(interface)
            .check(&["commit-message", "not conventional"])
            .expect_fail();
    }

    #[test_case(Cli)]
    #[test_case(CliStdin)]
    #[test_case(Mcp)]
    fn invalid_message_shows_subject_line_as_target(interface: Interface) {
        Scute::new(interface)
            .check(&["commit-message", "not conventional"])
            .expect_target("not conventional");
    }

    #[test_case(Cli)]
    #[test_case(CliStdin)]
    #[test_case(Mcp)]
    fn config_types_override_defaults(interface: Interface) {
        Scute::new(interface)
            .scute_config(
                r"
checks:
  commit-message:
    types: [hotfix]
",
            )
            .check(&["commit-message", "hotfix: urgent patch"])
            .expect_pass();
    }

    #[test_case(Cli)]
    #[test_case(CliStdin)]
    #[test_case(Mcp)]
    fn passing_check_omits_evidence(interface: Interface) {
        Scute::new(interface)
            .check(&["commit-message", "feat: add login"])
            .expect_pass()
            .expect_no_findings();
    }
}

mod dependency_freshness {
    use scute_test_utils::{Interface, Scute, TestProject};
    use test_case::test_case;

    use Interface::{Cli, Mcp};

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn fresh_project_passes(interface: Interface) {
        Scute::new(interface)
            .check(&["dependency-freshness"])
            .expect_pass();
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn outdated_deps_fail(interface: Interface) {
        Scute::new(interface)
            .dependency("itoa", "=0.4.8")
            .check(&["dependency-freshness"])
            .expect_fail();
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn nonexistent_path_produces_error(interface: Interface) {
        Scute::new(interface)
            .check(&["dependency-freshness", "/nonexistent/path"])
            .expect_error("invalid_target");
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn non_cargo_dir_produces_error(interface: Interface) {
        let dir = TestProject::empty().build();

        Scute::new(interface)
            .check(&["dependency-freshness", dir.path().to_str().unwrap()])
            .expect_error("invalid_target");
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn config_thresholds_override_defaults(interface: Interface) {
        Scute::new(interface)
            .dependency("itoa", "=0.4.8")
            .scute_config(
                r"
checks:
  dependency-freshness:
    thresholds:
      fail: 5
",
            )
            .check(&["dependency-freshness"])
            .expect_pass();
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn warn_between_thresholds(interface: Interface) {
        Scute::new(interface)
            .dependency("itoa", "=0.4.8")
            .scute_config(
                r"
checks:
  dependency-freshness:
    thresholds:
      warn: 0
      fail: 5
",
            )
            .check(&["dependency-freshness"])
            .expect_warn();
    }
}

mod code_similarity {
    use scute_test_utils::{Interface, Scute};
    use test_case::test_case;

    use Interface::{Cli, Mcp};

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn duplicated_code_reports_failure(interface: Interface) {
        Scute::new(interface)
            .source_file("a.rs", "fn foo(x: i32) -> i32 { x + 1 }")
            .source_file("b.rs", "fn bar(y: i32) -> i32 { y + 1 }")
            .scute_config(
                r"
checks:
  code-similarity:
    thresholds:
      fail: 0
    min-tokens: 5
",
            )
            .check(&["code-similarity"])
            .expect_fail();
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn exclude_patterns_skip_matching_files(interface: Interface) {
        Scute::new(interface)
            .source_file("a.rs", "fn foo(x: i32) -> i32 { x + 1 }")
            .source_file("b.rs", "fn bar(y: i32) -> i32 { y + 1 }")
            .scute_config(
                r"
checks:
  code-similarity:
    thresholds:
      fail: 0
    min-tokens: 5
    exclude:
      - 'b.rs'
",
            )
            .check(&["code-similarity"])
            .expect_pass();
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn nonexistent_source_dir_produces_error(interface: Interface) {
        Scute::new(interface)
            .check(&["code-similarity", "--source-dir", "/nonexistent/path"])
            .expect_error("invalid_target");
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn focus_file_filters_reported_clones(interface: Interface) {
        Scute::new(interface)
            .source_file("a.rs", "fn foo(x: i32) -> i32 { x + 1 }")
            .source_file("b.rs", "fn bar(y: i32) -> i32 { y + 1 }")
            .source_file("c.rs", "const A: [i32; 5] = [10, 20, 30, 40, 50];")
            .source_file("d.rs", "const B: [u32; 5] = [60, 70, 80, 90, 100];")
            .scute_config(
                r"
checks:
  code-similarity:
    thresholds:
      fail: 0
    min-tokens: 5
",
            )
            .check(&["code-similarity", "a.rs"])
            .expect_fail()
            .expect_finding_count(1)
            .expect_target_contains("a.rs");
    }
}

mod code_complexity {
    use scute_test_utils::{Interface, Scute};
    use test_case::test_case;

    use Interface::{Cli, Mcp};

    // for: +1, if: +2, if: +3, else: +1 → score 7, 4 contributors (1 nesting at index 1)
    const COMPLEX_SOURCE: &str = r"
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

    fn complex_function_check(interface: Interface) -> scute_test_utils::CheckResult {
        Scute::new(interface)
            .scute_config(
                r"
checks:
  code-complexity:
    thresholds:
      warn: 1
      fail: 10
",
            )
            .source_file("src/complex.rs", COMPLEX_SOURCE)
            .check(&["code-complexity"])
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn complex_rust_function_gets_flagged(interface: Interface) {
        complex_function_check(interface)
            .expect_warn()
            .expect_target_contains("process");
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn flagged_function_includes_evidence(interface: Interface) {
        complex_function_check(interface)
            .expect_warn()
            .expect_evidence_count(4)
            .expect_evidence_rule(1, "nesting");
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn simple_rust_function_passes(interface: Interface) {
        Scute::new(interface)
            .source_file("src/simple.rs", "fn add(a: i32, b: i32) -> i32 { a + b }")
            .check(&["code-complexity"])
            .expect_pass();
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn config_thresholds_override_defaults(interface: Interface) {
        Scute::new(interface)
            .source_file("src/complex.rs", COMPLEX_SOURCE)
            .scute_config(
                r"
checks:
  code-complexity:
    thresholds:
      warn: 20
      fail: 30
",
            )
            .check(&["code-complexity"])
            .expect_pass();
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn exclude_patterns_skip_matching_files(interface: Interface) {
        Scute::new(interface)
            .source_file("src/complex.rs", COMPLEX_SOURCE)
            .scute_config(
                r"
checks:
  code-complexity:
    thresholds:
      warn: 1
    exclude:
      - 'src/complex.rs'
",
            )
            .check(&["code-complexity"])
            .expect_pass();
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn complex_typescript_function_gets_flagged(interface: Interface) {
        Scute::new(interface)
            .scute_config(
                r"
checks:
  code-complexity:
    thresholds:
      warn: 1
      fail: 10
",
            )
            .source_file(
                "src/complex.ts",
                r"
function process(items: number[]): number {
    let total = 0;
    for (const item of items) {
        if (item > 0) {
            if (item > 10) {
                total += item;
            } else {
                total -= item;
            }
        }
    }
    return total;
}
",
            )
            .check(&["code-complexity"])
            .expect_warn()
            .expect_target_contains("process");
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn nonexistent_path_produces_error(interface: Interface) {
        Scute::new(interface)
            .check(&["code-complexity", "/nonexistent/path"])
            .expect_error("invalid_target");
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn scores_only_specified_file(interface: Interface) {
        Scute::new(interface)
            .source_file("src/complex.rs", COMPLEX_SOURCE)
            .source_file("src/simple.rs", "fn add(a: i32, b: i32) -> i32 { a + b }")
            .scute_config(
                r"
checks:
  code-complexity:
    thresholds:
      warn: 1
",
            )
            .check(&["code-complexity", "src/simple.rs"])
            .expect_pass();
    }
}

mod config {
    use scute_test_utils::{Interface, Scute};
    use test_case::test_case;

    use Interface::{Cli, Mcp};

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn malformed_config_produces_error(interface: Interface) {
        Scute::new(interface)
            .scute_config("not: valid: yaml: [")
            .check(&["commit-message", "feat: add login"])
            .expect_error("invalid_config");
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn empty_config_uses_defaults(interface: Interface) {
        Scute::new(interface)
            .scute_config("")
            .check(&["commit-message", "feat: add login"])
            .expect_pass();
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn invalid_check_config_produces_error(interface: Interface) {
        Scute::new(interface)
            .scute_config(
                r"
checks:
  commit-message:
    config:
      types: [hotfix]
",
            )
            .check(&["commit-message", "hotfix: urgent patch"])
            .expect_error("invalid_config");
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn picks_up_config_from_parent_directory(interface: Interface) {
        Scute::new(interface)
            .scute_config(
                r"
checks:
  commit-message:
    types: [hotfix]
",
            )
            .cwd("nested")
            .check(&["commit-message", "hotfix: urgent patch"])
            .expect_pass();
    }
}
