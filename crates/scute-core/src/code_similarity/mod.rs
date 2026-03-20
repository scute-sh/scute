mod check;
mod detect;
mod evaluate;
mod format;
pub mod javascript;
mod parse;
pub mod rules;
pub mod rust;
pub mod tree;

pub use check::{CHECK_NAME, Definition, check, languages};
pub use detect::{CloneGroup, Occurrence, detect_clones};
pub use parse::{ParseError, parse_source};
pub use tree::Token;

/// Shared test utilities for language rule tests.
#[cfg(test)]
pub(crate) mod test_support {
    use super::tree::{SourceTree, Token};
    use crate::files::SourceFile;

    pub fn parse_file(source: &str, path: &str) -> (SourceTree, Vec<Token>) {
        let file = SourceFile {
            path: path.to_string(),
            content: source.to_string(),
        };
        let tree = super::parse_source(&file, &super::check::languages()).unwrap();
        let tokens = tree.tokens();
        (tree, tokens)
    }

    pub fn token_texts(tokens: &[Token]) -> Vec<&str> {
        tokens.iter().map(|t| t.text.as_str()).collect()
    }
}
