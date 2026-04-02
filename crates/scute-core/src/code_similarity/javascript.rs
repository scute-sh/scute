use std::path::Path;

use crate::language::JsFamily;

use super::rules::{SimilarityRules, token};
use super::tree::NodeKind;

impl SimilarityRules for JsFamily {
    fn classify_file(&self, path: &Path) -> Option<NodeKind> {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let has_test_suffix = stem
            .rsplit_once('.')
            .is_some_and(|(_, suffix)| suffix == "test" || suffix == "spec");
        let in_tests_directory = path.components().any(|c| c.as_os_str() == "__tests__");

        if has_test_suffix || in_tests_directory {
            Some(NodeKind::TestRegion)
        } else {
            None
        }
    }

    fn classify_node(&self, node: tree_sitter::Node, src: &[u8]) -> Option<NodeKind> {
        match node.kind() {
            // Returns None for classes without heritage, letting the walker
            // recurse into the class body as unclassified tokens.
            "class_declaration" | "abstract_class_declaration" | "class" => {
                classify_class(&node, src)
            }

            "array" => Some(NodeKind::Collection),

            // Identifiers
            "identifier"
            | "shorthand_property_identifier"
            | "shorthand_property_identifier_pattern"
            | "property_identifier"
            | "type_identifier"
            | "predefined_type" => Some(token("$ID", &node)),

            // Literals
            "string" | "template_string" | "number" | "true" | "false" | "null" | "undefined"
            | "regex" => Some(token("$LIT", &node)),

            // Skip
            "comment" => Some(NodeKind::Comment),
            "decorator" => Some(NodeKind::Decoration),
            "import_statement" => Some(NodeKind::Import),

            _ => None,
        }
    }
}

/// Extract contract names from a class's heritage clause.
///
/// Covers `class_declaration`, `abstract_class_declaration`, and `class`
/// (expression). Collects all names from `implements` and `extends` clauses.
/// Returns None for classes without heritage, which the walker treats as
/// unclassified (recurse into body).
fn classify_class(node: &tree_sitter::Node, src: &[u8]) -> Option<NodeKind> {
    let mut cursor = node.walk();
    let heritage = node
        .children(&mut cursor)
        .find(|c| c.kind() == "class_heritage")?;

    let names = collect_contract_names(&heritage, src);
    if names.is_empty() {
        return None;
    }
    Some(NodeKind::Contract { names })
}

fn is_heritage_clause(kind: &str) -> bool {
    kind == "implements_clause" || kind == "extends_clause"
}

fn collect_contract_names(heritage: &tree_sitter::Node, src: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = heritage.walk();

    // TS path: collect names from implements_clause and extends_clause
    for child in heritage.children(&mut cursor) {
        if is_heritage_clause(child.kind()) {
            collect_type_names(&child, src, &mut names);
        }
    }

    // JS path: class_heritage directly contains the identifier
    if names.is_empty()
        && let Some(name) = first_type_name(heritage, src)
    {
        names.push(name);
    }

    names
}

/// Collect all type names from a clause (`implements_clause` or `extends_clause`).
///
/// Handles `implements A, B` by iterating all named children.
/// Strips generic type parameters so `Renderer<string>` becomes `Renderer`.
fn collect_type_names(clause: &tree_sitter::Node, src: &[u8], out: &mut Vec<String>) {
    let mut cursor = clause.walk();
    for child in clause.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if let Some(name) = extract_type_name(&child, src) {
            out.push(name);
        }
    }
}

/// Extract a type name from a single node, stripping generic parameters.
///
/// `Renderer` → "Renderer", `Renderer<string>` → "Renderer".
fn extract_type_name(node: &tree_sitter::Node, src: &[u8]) -> Option<String> {
    if node.kind() == "generic_type" {
        let mut cursor = node.walk();
        let base = node
            .children(&mut cursor)
            .find(tree_sitter::Node::is_named)?;
        return base.utf8_text(src).ok().map(String::from);
    }
    node.utf8_text(src).ok().map(String::from)
}

/// Extract the first type name from a node's named children.
///
/// Used for JS `class_heritage` which directly contains the identifier.
fn first_type_name(node: &tree_sitter::Node, src: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    let name_node = node
        .children(&mut cursor)
        .find(tree_sitter::Node::is_named)?;
    extract_type_name(&name_node, src)
}

#[cfg(test)]
mod tests {
    use crate::code_similarity::test_support::{parse_file, token_texts};
    use crate::code_similarity::tree::all_share_contract;

    #[test]
    fn classify_file_marks_dot_test_ts_as_test_region() {
        let (tree, tokens) = parse_file("function f() {}", "src/app.test.ts");
        assert!(tree.is_in_test_region(tokens[0].node_index));
    }

    #[test]
    fn classify_file_marks_dot_spec_js_as_test_region() {
        let (tree, tokens) = parse_file("function f() {}", "src/app.spec.js");
        assert!(tree.is_in_test_region(tokens[0].node_index));
    }

    #[test]
    fn classify_file_marks_dunder_tests_directory_as_test_region() {
        let (tree, tokens) = parse_file("function f() {}", "__tests__/foo.js");
        assert!(tree.is_in_test_region(tokens[0].node_index));
    }

    #[test]
    fn classify_file_ignores_regular_src_file() {
        let (tree, tokens) = parse_file("function f() {}", "src/app.ts");
        assert!(!tree.is_in_test_region(tokens[0].node_index));
    }

    #[test]
    fn classify_file_marks_dot_test_tsx_as_test_region() {
        let (tree, tokens) = parse_file("function f() {}", "src/Button.test.tsx");
        assert!(tree.is_in_test_region(tokens[0].node_index));
    }

    #[test]
    fn normalizes_identifiers_to_id_placeholder() {
        let (_, tokens) = parse_file("let x = y;", "a.ts");
        assert_eq!(token_texts(&tokens), vec!["let", "$ID", "=", "$ID", ";"]);
    }

    #[test]
    fn normalizes_type_identifiers() {
        let (_, tokens) = parse_file("let x: MyType = y;", "a.ts");
        assert_eq!(
            token_texts(&tokens),
            vec!["let", "$ID", ":", "$ID", "=", "$ID", ";"]
        );
    }

    #[test]
    fn normalizes_predefined_types() {
        let (_, tokens) = parse_file("let x: string = y;", "a.ts");
        assert_eq!(
            token_texts(&tokens),
            vec!["let", "$ID", ":", "$ID", "=", "$ID", ";"]
        );
    }

    #[test]
    fn normalizes_property_identifiers() {
        let (_, tokens) = parse_file("const o = { key: 1 };", "a.ts");
        assert!(token_texts(&tokens).contains(&"$ID"));
    }

    #[test]
    fn normalizes_shorthand_property_identifiers() {
        let (_, tokens) = parse_file("const o = { x };", "a.ts");
        // `x` is a shorthand_property_identifier → $ID
        assert_eq!(
            token_texts(&tokens),
            vec!["const", "$ID", "=", "{", "$ID", "}", ";"]
        );
    }

    #[test_case::test_case("\"hello\"", "a.ts" ; "string")]
    #[test_case::test_case("`hello`", "a.ts" ; "template string")]
    #[test_case::test_case("42", "a.ts" ; "number")]
    #[test_case::test_case("true", "a.ts" ; "boolean true")]
    #[test_case::test_case("null", "a.ts" ; "null")]
    #[test_case::test_case("undefined", "a.js" ; "undefined")]
    fn normalizes_literal_to_lit_placeholder(value: &str, file: &str) {
        let (_, tokens) = parse_file(&format!("const x = {value};"), file);
        assert_eq!(token_texts(&tokens), vec!["const", "$ID", "=", "$LIT", ";"]);
    }

    #[test]
    fn normalizes_regex_literal() {
        let (_, tokens) = parse_file("const r = /abc/g;", "a.js");
        assert!(token_texts(&tokens).contains(&"$LIT"));
    }

    #[test]
    fn strips_comments() {
        let (_, tokens) = parse_file("// a comment\nconst x = 1;", "a.ts");
        assert_eq!(token_texts(&tokens), vec!["const", "$ID", "=", "$LIT", ";"]);
    }

    #[test]
    fn strips_block_comments() {
        let (_, tokens) = parse_file("/* block */\nconst x = 1;", "a.ts");
        assert_eq!(token_texts(&tokens), vec!["const", "$ID", "=", "$LIT", ";"]);
    }

    #[test]
    fn strips_decorators() {
        let (_, tokens) = parse_file("@Component({})\nclass Foo {}", "a.ts");
        let texts = token_texts(&tokens);
        assert!(!texts.contains(&"@"));
        assert!(texts.contains(&"class"));
    }

    #[test_case::test_case("import { foo } from 'bar';", "a.js" ; "js named import")]
    #[test_case::test_case("import { foo } from 'bar';", "a.ts" ; "ts named import")]
    #[test_case::test_case("import { foo } from 'bar';", "a.tsx" ; "tsx named import")]
    #[test_case::test_case("import foo from 'bar';", "a.ts" ; "default import")]
    #[test_case::test_case("import * as foo from 'bar';", "a.ts" ; "namespace import")]
    fn strips_import_statements(source: &str, path: &str) {
        let (_, tokens) = parse_file(source, path);
        assert!(tokens.is_empty(), "expected no tokens, got: {tokens:?}");
    }

    #[test]
    fn preserves_code_after_imports() {
        let (_, tokens) = parse_file("import { foo } from 'bar';\nconst x = 1;", "a.ts");
        assert_eq!(token_texts(&tokens), vec!["const", "$ID", "=", "$LIT", ";"]);
    }

    #[test_case::test_case(
        "class Html implements Renderer { render(): string { return ''; } }",
        "a.ts", true ; "ts implements"
    )]
    #[test_case::test_case(
        "class Html extends AbstractRenderer { render(): string { return ''; } }",
        "a.ts", true ; "ts extends"
    )]
    #[test_case::test_case(
        "class Html extends Base { render() { return ''; } }",
        "a.js", true ; "js extends"
    )]
    #[test_case::test_case(
        "abstract class Base implements Renderer { abstract render(): string; }",
        "a.ts", true ; "ts abstract class"
    )]
    #[test_case::test_case(
        "class Html implements Renderer { render(): string { return ''; } }",
        "a.tsx", true ; "tsx implements"
    )]
    #[test_case::test_case(
        "class Html implements Renderer<string> { render(): string { return ''; } }",
        "a.ts", true ; "ts generic implements"
    )]
    #[test_case::test_case(
        "class Foo { bar(): string { return ''; } }",
        "a.ts", false ; "plain class"
    )]
    #[test_case::test_case(
        "function render(): string { return ''; }",
        "a.ts", false ; "free function"
    )]
    fn extracts_contract_from_class(source: &str, path: &str, has_contract: bool) {
        let (tree, tokens) = parse_file(source, path);
        assert_eq!(all_share_contract(&[(&tree, &tokens)]), has_contract);
    }

    #[test_case::test_case(
        "class A implements Renderer, Serializable { render(): string { return ''; } }",
        "class B implements Renderer { render(): string { return ''; } }"
        ; "multiple implements shares contract with single"
    )]
    #[test_case::test_case(
        "class A extends Base implements Renderer { render(): string { return ''; } }",
        "class B extends Base implements Formatter { format(): string { return ''; } }"
        ; "extends plus implements shares contract via either"
    )]
    #[test_case::test_case(
        "class A implements Renderer<string> { render(): string { return ''; } }",
        "class B implements Renderer<number> { render(): number { return 0; } }"
        ; "generic type params ignored in contract name"
    )]
    fn recognizes_shared_contract_across_files(source_a: &str, source_b: &str) {
        let (tree_a, tokens_a) = parse_file(source_a, "a.ts");
        let (tree_b, tokens_b) = parse_file(source_b, "b.ts");
        assert!(all_share_contract(&[
            (&tree_a, &tokens_a),
            (&tree_b, &tokens_b),
        ]));
    }
}
