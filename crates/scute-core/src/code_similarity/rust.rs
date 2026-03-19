use std::path::Path;

use super::rules::{SimilarityRules, token};
use super::tree::NodeKind;
use tree_sitter::Language;

pub struct Rust;

impl SimilarityRules for Rust {
    fn language(&self) -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn classify_file(&self, path: &Path) -> Option<NodeKind> {
        if path.components().any(|c| c.as_os_str() == "tests") {
            Some(NodeKind::TestRegion)
        } else {
            None
        }
    }

    fn classify_node(&self, node: tree_sitter::Node, src: &[u8]) -> Option<NodeKind> {
        match node.kind() {
            // Structural containers
            "impl_item" => classify_impl(&node, src),
            "mod_item" if has_preceding_attr(&node, src, is_cfg_test_attr) => {
                Some(NodeKind::TestRegion)
            }
            "function_item" if has_preceding_attr(&node, src, |t| t == "#[test]") => {
                Some(NodeKind::TestRegion)
            }

            // Identifiers
            "identifier"
            | "type_identifier"
            | "field_identifier"
            | "shorthand_field_identifier"
            | "primitive_type"
            | "lifetime"
            | "self"
            | "metavariable"
            | "crate"
            | "super" => Some(token("$ID", &node)),

            // Literals
            "string_literal" | "raw_string_literal" | "char_literal" | "integer_literal"
            | "float_literal" | "boolean_literal" => Some(token("$LIT", &node)),

            // Skip
            "line_comment" | "block_comment" => Some(NodeKind::Comment),
            "attribute_item" | "inner_attribute_item" => Some(NodeKind::Decoration),

            _ => None,
        }
    }
}

fn classify_impl(node: &tree_sitter::Node, src: &[u8]) -> Option<NodeKind> {
    let trait_node = node.child_by_field_name("trait")?;
    let name = trait_node.utf8_text(src).ok()?.to_string();
    Some(NodeKind::Contract { name })
}

/// Matches `#[cfg(test)]` and compound forms like `#[cfg(all(test, ...))]`,
/// but not negated forms like `#[cfg(not(test))]` or `#[cfg(not(any(test)))]`.
fn is_cfg_test_attr(attr_text: &str) -> bool {
    attr_text.starts_with("#[cfg(")
        && !attr_text.contains("not(")
        && (attr_text == "#[cfg(test)]"
            || attr_text.contains("(test,")
            || attr_text.contains("(test)")
            || attr_text.contains(", test)")
            || attr_text.contains(", test,"))
}

fn has_preceding_attr(node: &tree_sitter::Node, src: &[u8], pred: impl Fn(&str) -> bool) -> bool {
    let mut sibling = node.prev_sibling();
    while let Some(s) = sibling {
        if s.kind() != "attribute_item" {
            break;
        }
        if s.utf8_text(src).is_ok_and(&pred) {
            return true;
        }
        sibling = s.prev_sibling();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_similarity::test_support::{parse_with, token_texts};
    use crate::code_similarity::tree::all_share_contract;

    fn parse(
        source: &str,
        path: &str,
    ) -> (
        crate::code_similarity::tree::SourceTree,
        Vec<crate::code_similarity::Token>,
    ) {
        parse_with(source, path, &Rust)
    }

    #[test]
    fn classify_file_marks_tests_directory_as_test_region() {
        let (tree, tokens) = parse("fn f() {}", "tests/a.rs");
        assert!(tree.is_in_test_region(tokens[0].node_index));
    }

    #[test]
    fn classify_file_ignores_src_directory() {
        let (tree, tokens) = parse("fn f() {}", "src/a.rs");
        assert!(!tree.is_in_test_region(tokens[0].node_index));
    }

    #[test]
    fn trait_impl_creates_contract_container() {
        let (tree, tokens) = parse(
            "impl Render for Html {\n    fn render(&self) -> String { String::new() }\n}",
            "a.rs",
        );
        assert!(all_share_contract(&[(&tree, &tokens)]));
    }

    #[test]
    fn inherent_impl_has_no_contract() {
        let (tree, tokens) = parse(
            "impl Html {\n    fn render(&self) -> String { String::new() }\n}",
            "a.rs",
        );
        assert!(!all_share_contract(&[(&tree, &tokens)]));
    }

    #[test]
    fn free_function_has_no_contract() {
        let (tree, tokens) = parse("fn render() -> String { String::new() }", "a.rs");
        assert!(!all_share_contract(&[(&tree, &tokens)]));
    }

    #[test]
    fn cfg_test_module_creates_test_region() {
        let (tree, tokens) = parse(
            "fn production() -> i32 { 42 }\n\n#[cfg(test)]\nmod tests {\n    fn helper(x: i32) -> i32 { x + 1 }\n}\n",
            "src/lib.rs",
        );
        assert!(!tree.is_in_test_region(tokens[0].node_index));
        assert!(tree.is_in_test_region(tokens.last().unwrap().node_index));
    }

    #[test]
    fn naked_test_fn_creates_test_region() {
        let (tree, tokens) = parse(
            "fn production() -> i32 { 42 }\n\n#[test]\nfn test_something() {\n    assert_eq!(production(), 42);\n}\n",
            "src/lib.rs",
        );
        assert!(!tree.is_in_test_region(tokens[0].node_index));
        assert!(tree.is_in_test_region(tokens.last().unwrap().node_index));
    }

    #[test]
    fn compound_cfg_test_creates_test_region() {
        let (tree, tokens) = parse(
            "#[cfg(all(test, feature = \"integration\"))]\nmod integration_tests {\n    fn helper(x: i32) -> i32 { x + 1 }\n}\n",
            "src/lib.rs",
        );
        assert!(tree.is_in_test_region(tokens.last().unwrap().node_index));
    }

    #[test]
    fn cfg_any_test_creates_test_region() {
        let (tree, tokens) = parse(
            "#[cfg(any(test))]\nmod tests {\n    fn helper(x: i32) -> i32 { x + 1 }\n}\n",
            "src/lib.rs",
        );
        assert!(tree.is_in_test_region(tokens.last().unwrap().node_index));
    }

    #[test]
    fn cfg_not_test_is_not_a_test_region() {
        let (tree, tokens) = parse(
            "#[cfg(not(test))]\nmod prod_only {\n    fn helper() -> i32 { 42 }\n}\n",
            "src/lib.rs",
        );
        assert!(!tree.is_in_test_region(tokens.last().unwrap().node_index));
    }

    #[test]
    fn cfg_not_any_test_is_not_a_test_region() {
        let (tree, tokens) = parse(
            "#[cfg(not(any(test)))]\nmod prod_only {\n    fn helper() -> i32 { 42 }\n}\n",
            "src/lib.rs",
        );
        assert!(!tree.is_in_test_region(tokens.last().unwrap().node_index));
    }

    #[test]
    fn test_fn_nested_in_non_test_module_creates_test_region() {
        let (tree, tokens) = parse(
            "mod integration {\n    #[test]\n    fn test_flow() {\n        assert!(true);\n    }\n}\n",
            "src/lib.rs",
        );
        // The `assert` token (line 4) is inside the #[test] fn, so inside TestRegion.
        // The last token is `}` from the mod (line 6), which is outside.
        let assert_tok = tokens
            .iter()
            .find(|t| t.text == "$ID" && t.start_line == 4)
            .unwrap();
        assert!(tree.is_in_test_region(assert_tok.node_index));
    }

    #[test]
    fn multiple_attributes_before_test_fn() {
        let (tree, tokens) = parse(
            "#[test]\n#[should_panic]\nfn test_something() {\n    panic!(\"expected\");\n}\n",
            "src/lib.rs",
        );
        assert!(tree.is_in_test_region(tokens.last().unwrap().node_index));
    }

    #[test]
    fn normalizes_identifiers_and_literals() {
        let (_, tokens) = parse("let x: i32 = 42;", "a.rs");
        assert_eq!(
            token_texts(&tokens),
            vec!["let", "$ID", ":", "$ID", "=", "$LIT", ";"]
        );
    }

    #[test]
    fn strips_comments_and_attributes() {
        let (_, tokens) = parse("// comment\n#[derive(Debug)]\nfn f() {}", "a.rs");
        assert_eq!(token_texts(&tokens), vec!["fn", "$ID", "(", ")", "{", "}"]);
    }
}
