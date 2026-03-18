mod check;
mod detect;
mod evaluate;
mod format;
pub mod javascript;
mod parse;
pub mod rules;
pub mod rust;
pub mod tree;

pub use check::{CHECK_NAME, Definition, check};
pub use detect::{CloneGroup, Occurrence, detect_clones};
pub use parse::{ParseError, parse_source};
pub use tree::Token;

/// Shared test utilities for language rule tests.
#[cfg(test)]
pub(crate) mod test_support {
    use super::rules::SimilarityRules;
    use super::tree::{SourceTree, Token};

    pub fn parse_with(
        source: &str,
        path: &str,
        rules: &dyn SimilarityRules,
    ) -> (SourceTree, Vec<Token>) {
        let tree = super::parse_source(source, path, rules).unwrap();
        let tokens = tree.tokens();
        (tree, tokens)
    }

    pub fn token_texts(tokens: &[Token]) -> Vec<&str> {
        tokens.iter().map(|t| t.text.as_str()).collect()
    }
}
