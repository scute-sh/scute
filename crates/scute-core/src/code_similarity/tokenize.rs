use super::language::{LanguageConfig, NodeRole};
use crate::parser::AstParser;

/// A normalized token from source code.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
}

impl Token {
    fn new(text: &str, node: &tree_sitter::Node) -> Self {
        Self {
            text: text.to_string(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
        }
    }
}

#[derive(Debug)]
pub struct TokenizeError;

impl std::fmt::Display for TokenizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to produce a parse tree")
    }
}

impl std::error::Error for TokenizeError {}

/// Tokenize source code into a normalized token stream.
///
/// The parser is borrowed mutably and its language is reconfigured on
/// each call. Callers should reuse the same parser across calls to
/// benefit from buffer recycling.
///
/// # Errors
///
/// Returns `TokenizeError` if the parser fails to produce a parse tree.
pub fn tokenize(
    parser: &mut dyn AstParser,
    source: &str,
    config: &LanguageConfig,
) -> Result<Vec<Token>, TokenizeError> {
    let tree = parser
        .parse(source, config.language())
        .map_err(|_| TokenizeError)?;

    let mut tokens = Vec::new();
    collect_tokens(tree.root_node(), source.as_bytes(), config, &mut tokens);
    Ok(tokens)
}

fn collect_tokens(
    node: tree_sitter::Node,
    source: &[u8],
    config: &LanguageConfig,
    tokens: &mut Vec<Token>,
) {
    if node.is_error() || node.is_missing() {
        return;
    }

    if node.is_named() {
        match config.classify(node.kind()) {
            NodeRole::Identifier => {
                tokens.push(Token::new("$ID", &node));
                return;
            }
            NodeRole::Literal => {
                tokens.push(Token::new("$LIT", &node));
                return;
            }
            NodeRole::Comment | NodeRole::Decoration => return,
            NodeRole::Other => {
                if node.child_count() == 0 {
                    let text = node.utf8_text(source).unwrap_or("");
                    tokens.push(Token::new(text, &node));
                    return;
                }
            }
        }
    }

    if node.child_count() == 0 {
        tokens.push(Token::new(node.kind(), &node));
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_tokens(child, source, config, tokens);
    }
}
