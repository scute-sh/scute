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

enum TokenAction {
    /// Emit this token and stop recursing.
    Emit(Token),
    /// Skip this node entirely (comments, decorations).
    Skip,
    /// Recurse into children.
    Recurse,
}

fn classify_node(node: &tree_sitter::Node, source: &[u8], config: &LanguageConfig) -> TokenAction {
    if node.is_error() || node.is_missing() {
        return TokenAction::Skip;
    }

    if !node.is_named() {
        return classify_unnamed(node);
    }

    classify_by_role(node, source, config)
}

fn classify_unnamed(node: &tree_sitter::Node) -> TokenAction {
    if node.child_count() == 0 {
        TokenAction::Emit(Token::new(node.kind(), node))
    } else {
        TokenAction::Recurse
    }
}

fn classify_by_role(
    node: &tree_sitter::Node,
    source: &[u8],
    config: &LanguageConfig,
) -> TokenAction {
    match config.classify(node.kind()) {
        NodeRole::Identifier => TokenAction::Emit(Token::new("$ID", node)),
        NodeRole::Literal => TokenAction::Emit(Token::new("$LIT", node)),
        NodeRole::Comment | NodeRole::Decoration => TokenAction::Skip,
        NodeRole::Other if node.child_count() == 0 => {
            let text = node.utf8_text(source).unwrap_or("");
            TokenAction::Emit(Token::new(text, node))
        }
        NodeRole::Other => TokenAction::Recurse,
    }
}

fn collect_tokens(
    node: tree_sitter::Node,
    source: &[u8],
    config: &LanguageConfig,
    tokens: &mut Vec<Token>,
) {
    match classify_node(&node, source, config) {
        TokenAction::Emit(token) => tokens.push(token),
        TokenAction::Skip => {}
        TokenAction::Recurse => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_tokens(child, source, config, tokens);
            }
        }
    }
}
