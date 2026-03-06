use std::collections::HashMap;

#[derive(Debug)]
pub struct LanguageConfig {
    language: tree_sitter::Language,
    roles: HashMap<&'static str, NodeRole>,
}

impl LanguageConfig {
    fn new(language: tree_sitter::Language, table: &[(NodeRole, &[&'static str])]) -> Self {
        let mut roles = HashMap::new();
        for &(role, kinds) in table {
            for &kind in kinds {
                roles.insert(kind, role);
            }
        }
        Self { language, roles }
    }

    #[must_use]
    pub fn language(&self) -> &tree_sitter::Language {
        &self.language
    }

    #[must_use]
    pub fn classify(&self, kind: &str) -> NodeRole {
        self.roles.get(kind).copied().unwrap_or(NodeRole::Other)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeRole {
    Identifier,
    Literal,
    Comment,
    Decoration,
    Other,
}

#[must_use]
pub fn rust() -> LanguageConfig {
    LanguageConfig::new(
        tree_sitter_rust::LANGUAGE.into(),
        &[
            (
                NodeRole::Identifier,
                &[
                    "identifier",
                    "type_identifier",
                    "field_identifier",
                    "shorthand_field_identifier",
                    "primitive_type",
                    "lifetime",
                    "self",
                    "metavariable",
                    "crate",
                    "super",
                ],
            ),
            (
                NodeRole::Literal,
                &[
                    "string_literal",
                    "raw_string_literal",
                    "char_literal",
                    "integer_literal",
                    "float_literal",
                    "boolean_literal",
                ],
            ),
            (NodeRole::Comment, &["line_comment", "block_comment"]),
            (
                NodeRole::Decoration,
                &["attribute_item", "inner_attribute_item"],
            ),
        ],
    )
}

#[must_use]
pub fn typescript() -> LanguageConfig {
    LanguageConfig::new(
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        &[
            (
                NodeRole::Identifier,
                &[
                    "identifier",
                    "shorthand_property_identifier",
                    "shorthand_property_identifier_pattern",
                    "property_identifier",
                    "type_identifier",
                    "predefined_type",
                ],
            ),
            (
                NodeRole::Literal,
                &[
                    "string",
                    "template_string",
                    "number",
                    "true",
                    "false",
                    "null",
                    "undefined",
                    "regex",
                ],
            ),
            (NodeRole::Comment, &["comment"]),
            (NodeRole::Decoration, &["decorator"]),
        ],
    )
}
