use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use crate::parser::AstParser;

type TestDetector = fn(&mut dyn AstParser, &Path, &str, usize, usize) -> bool;
type ContractDetector = fn(&mut dyn AstParser, &str, usize, usize) -> Option<String>;

pub struct LanguageConfig {
    language: tree_sitter::Language,
    roles: HashMap<&'static str, NodeRole>,
    test_detector: TestDetector,
    contract_detector: ContractDetector,
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
        contract_detector: ContractDetector,
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
            contract_detector,
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

    /// Returns `true` if the given line range is inside test code.
    ///
    /// Detection is language-specific: some languages need to parse
    /// the source, others rely on file path conventions alone.
    #[must_use]
    pub fn is_test_context(
        &self,
        parser: &mut dyn AstParser,
        path: &Path,
        content: &str,
        start_line: usize,
        end_line: usize,
    ) -> bool {
        (self.test_detector)(parser, path, content, start_line, end_line)
    }

    /// Returns the contract name if the given line range is inside a
    /// contract implementation (trait impl, interface impl, class extends).
    ///
    /// Returns `None` for inherent impls, free functions, and languages
    /// that don't support contract detection yet.
    #[must_use]
    pub fn contract_name(
        &self,
        parser: &mut dyn AstParser,
        content: &str,
        start_line: usize,
        end_line: usize,
    ) -> Option<String> {
        (self.contract_detector)(parser, content, start_line, end_line)
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

fn rust_is_test(
    parser: &mut dyn AstParser,
    path: &Path,
    content: &str,
    start_line: usize,
    end_line: usize,
) -> bool {
    if path.components().any(|c| c.as_os_str() == "tests") {
        return true;
    }
    let ranges = rust_test_ranges(parser, content);
    ranges
        .iter()
        .any(|&(range_start, range_end)| start_line >= range_start && end_line <= range_end)
}

/// Finds line ranges of Rust test code: `#[cfg(test)] mod` blocks and `#[test]` functions.
fn rust_test_ranges(parser: &mut dyn AstParser, content: &str) -> Vec<(usize, usize)> {
    let Ok(tree) = parser.parse(content, &tree_sitter_rust::LANGUAGE.into()) else {
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
            "mod_item" if has_preceding_attr(&node, src, is_cfg_test_attr) => {
                push_range_with_attrs(&node, ranges);
            }
            "mod_item" => recurse_into_mod_body(node, src, ranges),
            "function_item" if has_preceding_attr(&node, src, |t| t == "#[test]") => {
                push_range_with_attrs(&node, ranges);
            }
            _ => {}
        }
    }
}

fn push_range_with_attrs(node: &tree_sitter::Node, ranges: &mut Vec<(usize, usize)>) {
    let start = first_preceding_attr_row(node).unwrap_or(node.start_position().row);
    ranges.push((start + 1, node.end_position().row + 1));
}

fn recurse_into_mod_body(node: tree_sitter::Node, src: &[u8], ranges: &mut Vec<(usize, usize)>) {
    if let Some(body) = node.child_by_field_name("body") {
        collect_test_ranges(body, src, ranges);
    }
}

/// Matches `#[cfg(test)]` and compound forms like `#[cfg(all(test, ...))]`,
/// but not `#[cfg(not(test))]`.
fn is_cfg_test_attr(attr_text: &str) -> bool {
    attr_text.starts_with("#[cfg(")
        && !attr_text.contains("not(test)")
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

fn no_contract(
    _parser: &mut dyn AstParser,
    _content: &str,
    _start_line: usize,
    _end_line: usize,
) -> Option<String> {
    None
}

/// Returns the contract (trait) name if the given line range falls inside
/// a Rust trait impl block (`impl Trait for Type { ... }`).
/// Returns `None` for inherent impls (`impl Type { ... }`) and free code.
fn rust_contract_name(
    parser: &mut dyn AstParser,
    content: &str,
    start_line: usize,
    end_line: usize,
) -> Option<String> {
    let tree = parser
        .parse(content, &tree_sitter_rust::LANGUAGE.into())
        .ok()?;

    let src = content.as_bytes();
    let mut contracts = vec![];
    collect_contract_ranges(tree.root_node(), src, &mut contracts);
    contracts
        .into_iter()
        .find(|(_, range_start, range_end)| start_line <= *range_end && end_line >= *range_start)
        .map(|(name, _, _)| name)
}

/// Collects `(trait_name, start_line, end_line)` for each `impl Trait for Type` block.
fn collect_contract_ranges(
    parent: tree_sitter::Node,
    src: &[u8],
    contracts: &mut Vec<(String, usize, usize)>,
) {
    let mut cursor = parent.walk();
    for node in parent.children(&mut cursor) {
        match node.kind() {
            "impl_item" => push_trait_impl(&node, src, contracts),
            "mod_item" => recurse_into_mod_body_contracts(node, src, contracts),
            _ => {}
        }
    }
}

fn push_trait_impl(
    node: &tree_sitter::Node,
    src: &[u8],
    contracts: &mut Vec<(String, usize, usize)>,
) {
    if let Some(name) = extract_rust_trait_name(node, src) {
        let start = node.start_position().row + 1;
        let end = node.end_position().row + 1;
        contracts.push((name, start, end));
    }
}

fn recurse_into_mod_body_contracts(
    node: tree_sitter::Node,
    src: &[u8],
    contracts: &mut Vec<(String, usize, usize)>,
) {
    if let Some(body) = node.child_by_field_name("body") {
        collect_contract_ranges(body, src, contracts);
    }
}

/// Extracts the trait name from `impl Trait for Type { ... }`.
/// Returns `None` for inherent impls (`impl Type { ... }`).
fn extract_rust_trait_name(impl_node: &tree_sitter::Node, src: &[u8]) -> Option<String> {
    // In tree-sitter-rust, `impl Trait for Type` has a "trait" field.
    // Inherent impls (`impl Type`) do not.
    let trait_node = impl_node.child_by_field_name("trait")?;
    Some(trait_node.utf8_text(src).ok()?.to_string())
}

/// Detects test files by JS/TS conventions: `*.test.*`, `*.spec.*`, or `__tests__/` directory.
fn js_is_test(
    _parser: &mut dyn AstParser,
    path: &Path,
    _content: &str,
    _start_line: usize,
    _end_line: usize,
) -> bool {
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
        rust_contract_name,
    )
}

const TS_ROLES: &[(NodeRole, &[&str])] = &[
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
];

#[must_use]
pub fn javascript() -> LanguageConfig {
    LanguageConfig::new(
        tree_sitter_javascript::LANGUAGE.into(),
        &[
            (
                NodeRole::Identifier,
                &[
                    "identifier",
                    "shorthand_property_identifier",
                    "shorthand_property_identifier_pattern",
                    "property_identifier",
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
        js_is_test,
        no_contract,
    )
}

#[must_use]
pub fn typescript() -> LanguageConfig {
    LanguageConfig::new(
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        TS_ROLES,
        js_is_test,
        no_contract,
    )
}

#[must_use]
pub fn typescript_tsx() -> LanguageConfig {
    LanguageConfig::new(
        tree_sitter_typescript::LANGUAGE_TSX.into(),
        TS_ROLES,
        js_is_test,
        no_contract,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TreeSitterParser;

    fn parse_rust_test_ranges(src: &str) -> Vec<(usize, usize)> {
        let mut parser = TreeSitterParser::new();
        rust_test_ranges(&mut parser, src)
    }

    #[test]
    fn rust_test_ranges_finds_cfg_test_module() {
        let src = "\
fn production() -> i32 { 42 }

#[cfg(test)]
mod tests {
    fn helper(x: i32) -> i32 { x + 1 }
}
";
        assert_eq!(parse_rust_test_ranges(src), vec![(3, 6)]);
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
        assert_eq!(parse_rust_test_ranges(src), vec![(3, 7)]);
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
        assert_eq!(parse_rust_test_ranges(src), vec![(1, 5)]);
    }

    #[test]
    fn rejects_cfg_not_test_module() {
        let src = "\
#[cfg(not(test))]
mod prod_only {
    fn helper() -> i32 { 42 }
}
";
        assert!(parse_rust_test_ranges(src).is_empty());
    }

    #[test]
    fn detects_compound_cfg_test_as_test_context() {
        let src = "\
#[cfg(all(test, feature = \"integration\"))]
mod integration_tests {
    fn helper(x: i32) -> i32 { x + 1 }
}
";
        assert_eq!(parse_rust_test_ranges(src), vec![(1, 4)]);
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
        assert_eq!(parse_rust_test_ranges(src), vec![(2, 5)]);
    }

    fn find_rust_contract(src: &str, start_line: usize, end_line: usize) -> Option<String> {
        let mut parser = TreeSitterParser::new();
        rust_contract_name(&mut parser, src, start_line, end_line)
    }

    #[test]
    fn rust_trait_impl_returns_trait_name() {
        let src = "\
struct Html;
impl Render for Html {
    fn render(&self) -> String {
        String::new()
    }
}
";
        assert_eq!(find_rust_contract(src, 3, 5), Some("Render".to_string()));
    }

    #[test]
    fn rust_inherent_impl_returns_none() {
        let src = "\
struct Html;
impl Html {
    fn render(&self) -> String {
        String::new()
    }
}
";
        assert_eq!(find_rust_contract(src, 3, 5), None);
    }

    #[test]
    fn rust_free_function_returns_none() {
        let src = "\
fn render() -> String {
    String::new()
}
";
        assert_eq!(find_rust_contract(src, 1, 3), None);
    }

    #[test]
    fn rust_trait_impl_matches_overlapping_range() {
        let src = "\
use crate::Render;
impl Render for Html {
    fn render(&self) -> String {
        String::new()
    }
}
";
        // Range 1-6 overlaps the impl at lines 2-6
        assert_eq!(find_rust_contract(src, 1, 6), Some("Render".to_string()));
    }
}
