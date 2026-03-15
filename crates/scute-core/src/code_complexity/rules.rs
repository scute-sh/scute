use tree_sitter::Language;

use super::score::{FlowConstruct, JumpKeyword, LogicalOp};

pub enum NodeRole {
    FlowConstruct(FlowConstruct),
    ElseClause,
    NestingBoundary,
    LogicalExpression,
    LabeledJump(JumpKeyword, String),
    RecursiveCall,
}

pub enum NestingKind {
    /// Increases nesting, part of enclosing function (closures, arrow functions).
    /// The construct appears in nesting chains.
    Inline(FlowConstruct),
    /// Increases nesting in outer function, scored independently at depth 0.
    Separate,
}

pub struct ScoringUnit<'a> {
    pub name: String,
    pub line: usize,
    pub node: tree_sitter::Node<'a>,
    pub receiver_type: Option<String>,
}

/// Maps a language's tree-sitter AST to cognitive complexity drivers.
///
/// The scoring algorithm is language-agnostic: it asks the language to identify
/// its own constructs (flow control, nesting boundaries, logical operators, etc.)
/// and applies the Sonar cognitive complexity rules uniformly.
pub trait LanguageRules {
    /// The tree-sitter grammar for this language.
    fn language(&self) -> Language;

    /// If this node is a scoreable function or method, return its metadata.
    fn scoring_unit<'a>(
        &self,
        node: tree_sitter::Node<'a>,
        src: &'a [u8],
    ) -> Option<ScoringUnit<'a>>;

    /// If this node is a flow control construct (`if`, `for`, `match`, etc.),
    /// return its cognitive role and language-specific label.
    /// These get a structural increment of `1 + nesting`.
    fn flow_construct(&self, node: tree_sitter::Node) -> Option<FlowConstruct>;

    /// Whether this node is an else clause (scores +1 as a hybrid increment).
    fn is_else_clause(&self, node: tree_sitter::Node) -> bool;

    /// Whether this else clause is actually an else-if (scored flat, no nesting penalty).
    fn is_else_if(&self, node: tree_sitter::Node) -> bool;

    /// If this node is a nesting boundary (closure, arrow function, nested
    /// named function), return how it affects scoring.
    fn nesting_kind(&self, node: tree_sitter::Node) -> Option<NestingKind>;

    /// If this node is a logical expression, return which operator it uses.
    /// Used to count operator sequences.
    fn logical_operator(&self, node: tree_sitter::Node) -> Option<LogicalOp>;

    /// Whether this node is a logical operator token (`&&`, `||`).
    /// Used to filter operands when walking logical expression trees.
    fn is_logical_operator_token(&self, node: tree_sitter::Node) -> bool;

    /// If this node is a labeled jump (`break 'label`, `continue 'label`),
    /// return the keyword and label text. Scores +1 as a hybrid increment.
    fn jump_label(&self, node: tree_sitter::Node, src: &[u8]) -> Option<(JumpKeyword, String)>;

    /// Whether this node is a direct recursive call to the function being scored.
    fn is_recursive_call(
        &self,
        node: tree_sitter::Node,
        fn_name: &str,
        receiver_type: Option<&str>,
        src: &[u8],
    ) -> bool;
}
