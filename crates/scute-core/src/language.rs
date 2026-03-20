//! Language definitions for tree-sitter based checks.
//!
//! A [`Language`] pairs a tree-sitter grammar with its file extensions.
//! [`LanguageRegistry`] maps file paths to the right language + check rules.
//! Check-specific traits ([`ComplexityRules`], [`SimilarityRules`]) are
//! implemented on the shared rule types [`Rust`] and [`JsFamily`].
//!
//! [`ComplexityRules`]: crate::code_complexity
//! [`SimilarityRules`]: crate::code_similarity::rules::SimilarityRules

use std::path::Path;

/// A tree-sitter grammar paired with its recognized file extensions.
pub struct Language {
    pub grammar: tree_sitter::Language,
    pub extensions: &'static [&'static str],
}

/// Extension-based registry mapping file extensions to language rules.
///
/// Both `code_similarity` and `code_complexity` use this to resolve
/// a file path to the right language + check-specific rules.
pub struct LanguageRegistry<R: ?Sized> {
    entries: Vec<LanguageRegistryEntry<R>>,
}

pub struct LanguageRegistryEntry<R: ?Sized> {
    pub language: Language,
    pub rules: Box<R>,
}

impl<R: ?Sized> LanguageRegistry<R> {
    #[must_use]
    pub fn new(entries: Vec<LanguageRegistryEntry<R>>) -> Self {
        Self { entries }
    }

    #[must_use]
    pub fn for_path(&self, path: &Path) -> Option<(&Language, &R)> {
        let ext = path.extension()?.to_str()?;
        self.entries
            .iter()
            .find(|e| e.language.extensions.contains(&ext))
            .map(|e| (&e.language, e.rules.as_ref()))
    }

    #[must_use]
    pub fn supported_extensions(&self) -> Vec<&'static str> {
        self.entries
            .iter()
            .flat_map(|e| e.language.extensions.iter().copied())
            .collect()
    }
}

/// Rule type for Rust source code.
pub struct Rust;

/// Rule type for the JavaScript/TypeScript family.
///
/// Covers JavaScript, TypeScript, and TSX. The grammar varies per variant
/// (JS vs TS vs TSX), but the AST node kinds used by checks are shared
/// across all three, so one type handles all variants.
pub struct JsFamily;

#[must_use]
pub fn rust() -> Language {
    Language {
        grammar: tree_sitter_rust::LANGUAGE.into(),
        extensions: &["rs"],
    }
}

#[must_use]
pub fn javascript() -> Language {
    Language {
        grammar: tree_sitter_javascript::LANGUAGE.into(),
        extensions: &["js", "jsx", "mjs", "cjs"],
    }
}

#[must_use]
pub fn typescript() -> Language {
    Language {
        grammar: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        extensions: &["ts"],
    }
}

#[must_use]
pub fn typescript_tsx() -> Language {
    Language {
        grammar: tree_sitter_typescript::LANGUAGE_TSX.into(),
        extensions: &["tsx"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn test_registry() -> LanguageRegistry<&'static str> {
        LanguageRegistry::new(vec![
            LanguageRegistryEntry {
                language: Language {
                    grammar: tree_sitter_rust::LANGUAGE.into(),
                    extensions: &["rs"],
                },
                rules: Box::new("rust"),
            },
            LanguageRegistryEntry {
                language: Language {
                    grammar: tree_sitter_javascript::LANGUAGE.into(),
                    extensions: &["js", "jsx"],
                },
                rules: Box::new("js"),
            },
        ])
    }

    #[test]
    fn resolves_matching_extension() {
        let reg = test_registry();
        let (_, rules) = reg.for_path(Path::new("main.rs")).unwrap();
        assert_eq!(*rules, "rust");
    }

    #[test]
    fn resolves_second_extension_in_list() {
        let reg = test_registry();
        let (_, rules) = reg.for_path(Path::new("app.jsx")).unwrap();
        assert_eq!(*rules, "js");
    }

    #[test]
    fn returns_none_for_unsupported_extension() {
        let reg = test_registry();
        assert!(reg.for_path(Path::new("style.css")).is_none());
    }

    #[test]
    fn returns_none_for_no_extension() {
        let reg = test_registry();
        assert!(reg.for_path(Path::new("Makefile")).is_none());
    }

    #[test]
    fn empty_registry_returns_none() {
        let reg: LanguageRegistry<&str> = LanguageRegistry::new(vec![]);
        assert!(reg.for_path(Path::new("main.rs")).is_none());
    }

    #[test]
    fn supported_extensions_collects_all() {
        let reg = test_registry();
        let exts = reg.supported_extensions();
        assert_eq!(exts, vec!["rs", "js", "jsx"]);
    }
}
