use tree_sitter::{Language, Tree};

/// A source code parser that produces syntax trees.
///
/// Implementations are stateful and mutable: a single instance should
/// be reused across multiple files so it can recycle internal buffers.
///
/// The signature currently uses [`tree_sitter::Language`] and
/// [`tree_sitter::Tree`] directly to avoid a wrapper layer that would
/// add no value today. Implementors must depend on the same
/// `tree_sitter` version.
pub trait AstParser {
    /// Parse source code with the given language grammar.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::LanguageSetup`] if the grammar can't be loaded,
    /// or [`ParseError::ParseFailed`] if the parser produces no tree.
    fn parse(&mut self, source: &str, language: &Language) -> Result<Tree, ParseError>;
}

#[derive(Debug)]
pub enum ParseError {
    /// The grammar could not be loaded (ABI version mismatch).
    LanguageSetup,
    /// The parser failed to produce a syntax tree.
    ParseFailed,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LanguageSetup => write!(f, "failed to load grammar (ABI mismatch)"),
            Self::ParseFailed => write!(f, "parser failed to produce a syntax tree"),
        }
    }
}

impl std::error::Error for ParseError {}

/// [`AstParser`] backed by tree-sitter.
pub struct TreeSitterParser(tree_sitter::Parser);

impl TreeSitterParser {
    #[must_use]
    pub fn new() -> Self {
        Self(tree_sitter::Parser::new())
    }
}

impl Default for TreeSitterParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AstParser for TreeSitterParser {
    fn parse(&mut self, source: &str, language: &Language) -> Result<Tree, ParseError> {
        self.0
            .set_language(language)
            .map_err(|_| ParseError::LanguageSetup)?;
        self.0.parse(source, None).ok_or(ParseError::ParseFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_rust_source() {
        let mut parser = TreeSitterParser::new();

        let tree = parser.parse("fn main() {}", &tree_sitter_rust::LANGUAGE.into());

        assert!(tree.is_ok());
    }

    #[test]
    fn switches_language_between_calls() {
        let mut parser = TreeSitterParser::new();

        let rust = parser.parse("fn main() {}", &tree_sitter_rust::LANGUAGE.into());
        let ts = parser.parse(
            "function main() {}",
            &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        );

        assert!(rust.is_ok());
        assert!(ts.is_ok());
    }
}
