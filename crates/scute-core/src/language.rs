//! Language definitions for tree-sitter based checks.
//!
//! A [`Language`] is a named tree-sitter grammar with its file extensions.
//! [`LanguageRegistry`] maps file paths to the right language + check rules.
//! Check-specific traits ([`ComplexityRules`], [`SimilarityRules`]) are
//! implemented on the shared rule types [`Rust`] and [`JsFamily`].
//!
//! [`ComplexityRules`]: crate::code_complexity
//! [`SimilarityRules`]: crate::code_similarity::rules::SimilarityRules

use std::path::Path;

/// A language as people name it, with the grammar and file extensions to
/// parse it.
pub struct Language {
    /// What people call the language. Variants share it: TypeScript and TSX
    /// are both "TypeScript".
    pub name: &'static str,
    pub grammar: tree_sitter::Language,
    pub extensions: &'static [&'static str],
}

/// The language families a check supports, each with that check's rules.
///
/// Resolves a file path, by extension, to its language and the check's
/// rules for it. Each check builds its own, registering only the families
/// it supports.
pub struct LanguageRegistry<R: ?Sized> {
    families: Vec<LanguageFamily<R>>,
}

/// Languages that share one set of check rules.
struct LanguageFamily<R: ?Sized> {
    languages: Vec<Language>,
    rules: Box<R>,
}

impl<R: ?Sized> Default for LanguageRegistry<R> {
    fn default() -> Self {
        Self { families: vec![] }
    }
}

impl<R: ?Sized> LanguageRegistry<R> {
    /// Supports `languages`, applying `rules` to each of them.
    #[must_use]
    pub fn with(mut self, rules: Box<R>, languages: Vec<Language>) -> Self {
        self.families.push(LanguageFamily { languages, rules });
        self
    }

    fn languages(&self) -> impl Iterator<Item = (&Language, &R)> {
        self.families.iter().flat_map(|family| {
            family
                .languages
                .iter()
                .map(|language| (language, family.rules.as_ref()))
        })
    }

    #[must_use]
    pub fn for_path(&self, path: &Path) -> Option<(&Language, &R)> {
        let ext = path.extension()?.to_str()?;
        self.languages()
            .find(|(language, _)| language.extensions.contains(&ext))
    }

    /// Each supported language's name, once, in registration order.
    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        for (language, _) in self.languages() {
            if !names.contains(&language.name) {
                names.push(language.name);
            }
        }
        names
    }

    #[must_use]
    pub fn supported_extensions(&self) -> Vec<&'static str> {
        self.languages()
            .flat_map(|(language, _)| language.extensions.iter().copied())
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

/// The languages [`Rust`] rules apply to.
#[must_use]
pub fn rust_family() -> Vec<Language> {
    vec![Language {
        name: "Rust",
        grammar: tree_sitter_rust::LANGUAGE.into(),
        extensions: &["rs"],
    }]
}

/// The languages [`JsFamily`] rules apply to.
#[must_use]
pub fn js_family() -> Vec<Language> {
    vec![
        Language {
            name: "JavaScript",
            grammar: tree_sitter_javascript::LANGUAGE.into(),
            extensions: &["js", "jsx", "mjs", "cjs"],
        },
        Language {
            name: "TypeScript",
            grammar: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            extensions: &["ts"],
        },
        Language {
            name: "TypeScript",
            grammar: tree_sitter_typescript::LANGUAGE_TSX.into(),
            extensions: &["tsx"],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn test_registry() -> LanguageRegistry<&'static str> {
        LanguageRegistry::default()
            .with(Box::new("rust"), vec![language("Rust", &["rs"])])
            .with(Box::new("js"), vec![language("JavaScript", &["js", "jsx"])])
    }

    fn language(name: &'static str, extensions: &'static [&'static str]) -> Language {
        Language {
            name,
            grammar: tree_sitter_javascript::LANGUAGE.into(),
            extensions,
        }
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
        let reg: LanguageRegistry<&str> = LanguageRegistry::default();
        assert!(reg.for_path(Path::new("main.rs")).is_none());
    }

    #[test]
    fn supported_extensions_collects_all() {
        let reg = test_registry();
        let exts = reg.supported_extensions();
        assert_eq!(exts, vec!["rs", "js", "jsx"]);
    }

    #[test]
    fn names_each_language_once_in_registration_order() {
        let reg = LanguageRegistry::default().with(
            Box::new("js"),
            vec![
                language("TypeScript", &["ts"]),
                language("JavaScript", &["js"]),
                language("TypeScript", &["tsx"]),
            ],
        );

        let names = reg.names();

        assert_eq!(names, vec!["TypeScript", "JavaScript"]);
    }
}
