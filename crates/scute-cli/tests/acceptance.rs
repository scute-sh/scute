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
    fn target_matches_argument(interface: Interface) {
        Scute::new(interface)
            .check(&["commit-message", "feat: from argument"])
            .expect_target("feat: from argument");
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
    config:
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
            .expect_no_evidences();
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
    fn uses_working_directory_as_target(interface: Interface) {
        Scute::new(interface)
            .check(&["dependency-freshness"])
            .expect_target_matches_dir();
    }

    #[test_case(Cli)]
    #[test_case(Mcp)]
    fn path_argument_resolves_target(interface: Interface) {
        let project = TestProject::cargo().build();
        let canonical = project.path().canonicalize().unwrap();

        Scute::new(interface)
            .check(&["dependency-freshness", project.path().to_str().unwrap()])
            .expect_pass()
            .expect_target(canonical.to_str().unwrap());
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
}
