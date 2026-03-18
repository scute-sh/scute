use std::path::Path;

use super::rules::{SimilarityRules, token};
use super::tree::NodeKind;
use tree_sitter::Language;

/// Covers JavaScript, TypeScript, and TSX (parameterized by grammar).
pub struct JsFamily {
    language: Language,
}

impl JsFamily {
    #[must_use]
    pub fn javascript() -> Self {
        Self {
            language: tree_sitter_javascript::LANGUAGE.into(),
        }
    }

    #[must_use]
    pub fn typescript() -> Self {
        Self {
            language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }
    }

    #[must_use]
    pub fn typescript_tsx() -> Self {
        Self {
            language: tree_sitter_typescript::LANGUAGE_TSX.into(),
        }
    }
}

impl SimilarityRules for JsFamily {
    fn language(&self) -> Language {
        self.language.clone()
    }

    fn classify_file(&self, path: &Path) -> Option<NodeKind> {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let is_test = Path::new(stem)
            .extension()
            .is_some_and(|ext| ext == "test" || ext == "spec")
            || path.components().any(|c| c.as_os_str() == "__tests__");

        if is_test {
            Some(NodeKind::TestRegion)
        } else {
            None
        }
    }

    fn classify_node(&self, node: tree_sitter::Node, _src: &[u8]) -> Option<NodeKind> {
        match node.kind() {
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

            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::code_similarity::test_support::{parse_with, token_texts};

    fn parse(
        source: &str,
        path: &str,
    ) -> (
        crate::code_similarity::tree::SourceTree,
        Vec<crate::code_similarity::Token>,
    ) {
        let lang = match Path::new(path).extension().and_then(|e| e.to_str()) {
            Some("js" | "jsx") => JsFamily::javascript(),
            Some("tsx") => JsFamily::typescript_tsx(),
            _ => JsFamily::typescript(),
        };
        parse_with(source, path, &lang)
    }

    // -- classify_file --

    #[test]
    fn classify_file_marks_dot_test_ts_as_test_region() {
        let (tree, tokens) = parse("function f() {}", "src/app.test.ts");
        assert!(tree.is_in_test_region(tokens[0].node_index));
    }

    #[test]
    fn classify_file_marks_dot_spec_js_as_test_region() {
        let (tree, tokens) = parse("function f() {}", "src/app.spec.js");
        assert!(tree.is_in_test_region(tokens[0].node_index));
    }

    #[test]
    fn classify_file_marks_dunder_tests_directory_as_test_region() {
        let (tree, tokens) = parse("function f() {}", "__tests__/foo.js");
        assert!(tree.is_in_test_region(tokens[0].node_index));
    }

    #[test]
    fn classify_file_ignores_regular_src_file() {
        let (tree, tokens) = parse("function f() {}", "src/app.ts");
        assert!(!tree.is_in_test_region(tokens[0].node_index));
    }

    #[test]
    fn classify_file_marks_dot_test_tsx_as_test_region() {
        let (tree, tokens) = parse("function f() {}", "src/Button.test.tsx");
        assert!(tree.is_in_test_region(tokens[0].node_index));
    }

    // -- token classification: identifiers --

    #[test]
    fn normalizes_identifiers_to_id_placeholder() {
        let (_, tokens) = parse("let x = y;", "a.ts");
        assert_eq!(token_texts(&tokens), vec!["let", "$ID", "=", "$ID", ";"]);
    }

    #[test]
    fn normalizes_type_identifiers() {
        let (_, tokens) = parse("let x: MyType = y;", "a.ts");
        assert_eq!(
            token_texts(&tokens),
            vec!["let", "$ID", ":", "$ID", "=", "$ID", ";"]
        );
    }

    #[test]
    fn normalizes_predefined_types() {
        let (_, tokens) = parse("let x: string = y;", "a.ts");
        assert_eq!(
            token_texts(&tokens),
            vec!["let", "$ID", ":", "$ID", "=", "$ID", ";"]
        );
    }

    #[test]
    fn normalizes_property_identifiers() {
        let (_, tokens) = parse("const o = { key: 1 };", "a.ts");
        assert!(token_texts(&tokens).contains(&"$ID"));
    }

    #[test]
    fn normalizes_shorthand_property_identifiers() {
        let (_, tokens) = parse("const o = { x };", "a.ts");
        // `x` is a shorthand_property_identifier → $ID
        assert_eq!(
            token_texts(&tokens),
            vec!["const", "$ID", "=", "{", "$ID", "}", ";"]
        );
    }

    // -- token classification: literals --

    #[test_case::test_case("\"hello\"", "a.ts" ; "string")]
    #[test_case::test_case("`hello`", "a.ts" ; "template string")]
    #[test_case::test_case("42", "a.ts" ; "number")]
    #[test_case::test_case("true", "a.ts" ; "boolean true")]
    #[test_case::test_case("null", "a.ts" ; "null")]
    #[test_case::test_case("undefined", "a.js" ; "undefined")]
    fn normalizes_literal_to_lit_placeholder(value: &str, file: &str) {
        let (_, tokens) = parse(&format!("const x = {value};"), file);
        assert_eq!(token_texts(&tokens), vec!["const", "$ID", "=", "$LIT", ";"]);
    }

    #[test]
    fn normalizes_regex_literal() {
        let (_, tokens) = parse("const r = /abc/g;", "a.js");
        assert!(token_texts(&tokens).contains(&"$LIT"));
    }

    // -- token classification: stripped nodes --

    #[test]
    fn strips_comments() {
        let (_, tokens) = parse("// a comment\nconst x = 1;", "a.ts");
        assert_eq!(token_texts(&tokens), vec!["const", "$ID", "=", "$LIT", ";"]);
    }

    #[test]
    fn strips_block_comments() {
        let (_, tokens) = parse("/* block */\nconst x = 1;", "a.ts");
        assert_eq!(token_texts(&tokens), vec!["const", "$ID", "=", "$LIT", ";"]);
    }

    #[test]
    fn strips_decorators() {
        let (_, tokens) = parse("@Component({})\nclass Foo {}", "a.ts");
        let texts = token_texts(&tokens);
        assert!(!texts.contains(&"@"));
        assert!(texts.contains(&"class"));
    }
}
