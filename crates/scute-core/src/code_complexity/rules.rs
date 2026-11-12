#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Construct {
    Conditional,
    Loop,
    ExceptionHandler,
    InlineNesting,
}

impl Construct {
    pub fn flow_break_category(self) -> &'static str {
        match self {
            Self::Conditional => "conditional",
            Self::Loop => "loop",
            Self::ExceptionHandler => "exception handler",
            Self::InlineNesting => "closure",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowConstruct {
    pub role: Construct,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpKeyword {
    Break,
    Continue,
}

impl JumpKeyword {
    pub fn label(self) -> &'static str {
        match self {
            Self::Break => "break",
            Self::Continue => "continue",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LogicalOp {
    And(&'static str),
    Or(&'static str),
}

impl LogicalOp {
    pub fn label(self) -> &'static str {
        match self {
            Self::And(s) | Self::Or(s) => s,
        }
    }
}

impl PartialEq for LogicalOp {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::And(_), Self::And(_)) | (Self::Or(_), Self::Or(_))
        )
    }
}

impl Eq for LogicalOp {}

#[derive(Clone, Copy)]
pub enum NestingKind {
    /// Increases nesting, part of enclosing function (closures, arrow functions).
    /// The construct appears in nesting chains.
    Inline(FlowConstruct),
    /// Increases nesting in outer function, scored independently at depth 0.
    Separate,
}

/// An `if`, split into the parts that score at different nesting levels: the
/// condition sits at the `if`'s own level, the consequence one level deeper.
#[derive(Clone, Copy)]
pub struct IfConstruct<'a> {
    pub condition: tree_sitter::Node<'a>,
    pub consequence: tree_sitter::Node<'a>,
    pub alternative: Option<ElseBranch<'a>>,
}

/// The `else` of an `if`, a hybrid increment of +1. Its body sits one level
/// deeper, like the consequence. When the body is itself an `if`, the pair
/// reads as one `else if`: the chained `if` takes no increment of its own and
/// its condition stays at the outer `if`'s level.
#[derive(Clone, Copy)]
pub struct ElseBranch<'a> {
    /// The `else` keyword, which locates the increment.
    pub keyword: tree_sitter::Node<'a>,
    /// What follows the keyword: a block, a single statement, or an `if`.
    pub body: tree_sitter::Node<'a>,
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
pub trait ComplexityRules {
    /// If this node is a scoreable function or method, return its metadata.
    fn scoring_unit<'a>(
        &self,
        node: tree_sitter::Node<'a>,
        src: &'a [u8],
    ) -> Option<ScoringUnit<'a>>;

    /// If this node is an `if`, return its parts.
    fn if_construct<'a>(&self, node: tree_sitter::Node<'a>) -> Option<IfConstruct<'a>>;

    /// If this node is any other flow control construct (`for`, `match`,
    /// ternary, etc.), return its cognitive role and language-specific label.
    /// These get a structural increment of `1 + nesting`, and everything
    /// inside them sits one level deeper.
    fn flow_construct(&self, node: tree_sitter::Node) -> Option<FlowConstruct>;

    /// If this node is a nesting boundary (closure, arrow function, nested
    /// named function), return how it affects scoring.
    fn nesting_kind(&self, node: tree_sitter::Node) -> Option<NestingKind>;

    /// If this node is a logical expression, return which operator it uses.
    /// Used to count operator sequences.
    fn logical_operator(&self, node: tree_sitter::Node) -> Option<LogicalOp>;

    /// Whether this node is a logical operator token (`&&`, `||`).
    /// Used to filter operands when walking logical expression trees.
    fn is_logical_operator_token(&self, node: tree_sitter::Node) -> bool;

    /// The expression this node groups, looking through any redundant
    /// parentheses, or the node itself when it isn't parenthesized.
    /// Parentheses don't break a sequence of like logical operators.
    fn skip_parentheses<'a>(&self, node: tree_sitter::Node<'a>) -> tree_sitter::Node<'a>;

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
