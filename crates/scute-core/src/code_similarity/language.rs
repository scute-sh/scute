use std::collections::HashMap;
use std::fmt;
use std::path::Path;

type TestDetector = fn(&Path, &str, usize, usize) -> bool;

pub struct LanguageConfig {
    language: tree_sitter::Language,
    roles: HashMap<&'static str, NodeRole>,
    test_detector: TestDetector,
}

impl fmt::Debug for LanguageConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LanguageConfig")
            .field("roles", &self.roles)
            .finish_non_exhaustive()
    }
}

impl LanguageConfig {
    fn new(
        language: tree_sitter::Language,
        table: &[(NodeRole, &[&'static str])],
        test_detector: TestDetector,
    ) -> Self {
        let mut roles = HashMap::new();
        for &(role, kinds) in table {
            for &kind in kinds {
                roles.insert(kind, role);
            }
        }
        Self {
            language,
            roles,
            test_detector,
        }
    }

    #[must_use]
    pub fn language(&self) -> &tree_sitter::Language {
        &self.language
    }

    #[must_use]
    pub fn classify(&self, kind: &str) -> NodeRole {
        self.roles.get(kind).copied().unwrap_or(NodeRole::Other)
    }

    #[must_use]
    pub fn is_test_context(
        &self,
        path: &Path,
        content: &str,
        start_line: usize,
        end_line: usize,
    ) -> bool {
        (self.test_detector)(path, content, start_line, end_line)
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

fn rust_is_test(path: &Path, content: &str, start_line: usize, end_line: usize) -> bool {
    if path.components().any(|c| c.as_os_str() == "tests") {
        return true;
    }
    let ranges = rust_test_ranges(content);
    ranges
        .iter()
        .any(|&(range_start, range_end)| start_line >= range_start && end_line <= range_end)
}

/// Finds line ranges of Rust test code: `#[cfg(test)] mod` blocks and `#[test]` functions.
fn rust_test_ranges(content: &str) -> Vec<(usize, usize)> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("rust grammar");
    let Some(tree) = parser.parse(content, None) else {
        return vec![];
    };

    let src = content.as_bytes();
    let mut ranges = vec![];
    collect_test_ranges(tree.root_node(), src, &mut ranges);
    ranges
}

fn collect_test_ranges(parent: tree_sitter::Node, src: &[u8], ranges: &mut Vec<(usize, usize)>) {
    let mut cursor = parent.walk();
    for node in parent.children(&mut cursor) {
        match node.kind() {
            "mod_item" if has_preceding_attr(&node, src, |t| t.contains("cfg(test)")) => {
                let start = first_preceding_attr_row(&node).unwrap_or(node.start_position().row);
                ranges.push((start + 1, node.end_position().row + 1));
            }
            "mod_item" => {
                let mut inner = node.walk();
                for child in node.children(&mut inner) {
                    if child.kind() == "declaration_list" {
                        collect_test_ranges(child, src, ranges);
                    }
                }
            }
            "function_item" if has_preceding_attr(&node, src, |t| t == "#[test]") => {
                let start = first_preceding_attr_row(&node).unwrap_or(node.start_position().row);
                ranges.push((start + 1, node.end_position().row + 1));
            }
            _ => {}
        }
    }
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

fn first_preceding_attr_row(node: &tree_sitter::Node) -> Option<usize> {
    let mut first_row = None;
    let mut sibling = node.prev_sibling();
    while let Some(s) = sibling {
        if s.kind() != "attribute_item" {
            break;
        }
        first_row = Some(s.start_position().row);
        sibling = s.prev_sibling();
    }
    first_row
}

fn ts_is_test(path: &Path, _content: &str, _start_line: usize, _end_line: usize) -> bool {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    Path::new(stem)
        .extension()
        .is_some_and(|ext| ext == "test" || ext == "spec")
        || path.components().any(|c| c.as_os_str() == "__tests__")
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
        rust_is_test,
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
        ts_is_test,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_test_ranges_finds_cfg_test_module() {
        let src = "\
fn production() -> i32 { 42 }

#[cfg(test)]
mod tests {
    fn helper(x: i32) -> i32 { x + 1 }
}
";
        let ranges = rust_test_ranges(src);

        assert_eq!(ranges, vec![(3, 6)]);
    }

    #[test]
    fn detects_naked_test_fn_as_test_context() {
        let src = "\
fn production() -> i32 { 42 }

#[test]
fn test_something() {
    let x = production();
    assert_eq!(x, 42);
}
";
        let ranges = rust_test_ranges(src);

        assert_eq!(ranges, vec![(3, 7)]);
    }

    #[test]
    fn walks_past_multiple_attributes_to_find_test() {
        let src = "\
#[test]
#[should_panic]
fn test_something() {
    panic!(\"expected\");
}
";
        let ranges = rust_test_ranges(src);

        assert_eq!(ranges, vec![(1, 5)]);
    }

    #[test]
    fn rejects_cfg_not_test_module() {
        let src = "\
#[cfg(not(test))]
mod prod_only {
    fn helper() -> i32 { 42 }
}
";
        let ranges = rust_test_ranges(src);

        assert!(ranges.is_empty());
    }

    #[test]
    fn finds_test_fn_nested_in_non_test_module() {
        let src = "\
mod integration {
    #[test]
    fn test_flow() {
        assert!(true);
    }
}
";
        let ranges = rust_test_ranges(src);

        assert_eq!(ranges, vec![(2, 5)]);
    }
}
