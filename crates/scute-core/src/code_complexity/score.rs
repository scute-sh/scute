use crate::parser::{AstParser, TreeSitterParser};

use super::rules::{LanguageRules, NestingKind, NodeRole};

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

#[derive(Debug, Clone, PartialEq)]
pub enum ContributorKind {
    FlowBreak {
        construct: FlowConstruct,
    },
    Nesting {
        construct: FlowConstruct,
        depth: u64,
        chain: Vec<FlowConstruct>,
    },
    Else,
    Logical {
        operators: Vec<LogicalOp>,
    },
    Recursion {
        fn_name: String,
    },
    Jump {
        keyword: JumpKeyword,
        label: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Contributor {
    pub kind: ContributorKind,
    pub line: usize,
    pub increment: u64,
}

pub struct FunctionScore {
    pub name: String,
    pub line: usize,
    pub score: u64,
    pub contributors: Vec<Contributor>,
}

pub fn score_functions(source: &str, rules: &dyn LanguageRules) -> Vec<FunctionScore> {
    let mut parser = TreeSitterParser::new();
    let Ok(tree) = parser.parse(source, &rules.language()) else {
        return vec![];
    };

    let src = source.as_bytes();
    let mut results = vec![];
    collect_functions(tree.root_node(), src, rules, &mut results);
    results
}

fn collect_functions(
    node: tree_sitter::Node,
    src: &[u8],
    rules: &dyn LanguageRules,
    results: &mut Vec<FunctionScore>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(unit) = rules.scoring_unit(child, src) {
            let mut contributors = vec![];
            let score = ScoringContext {
                rules,
                fn_name: &unit.name,
                impl_type: unit.receiver_type.as_deref(),
                src,
                contributors: &mut contributors,
            }
            .complexity(unit.node, 0);
            results.push(FunctionScore {
                name: unit.name,
                line: unit.line,
                score,
                contributors,
            });
        }
        collect_functions(child, src, rules, results);
    }
}

fn classify(
    rules: &dyn LanguageRules,
    node: tree_sitter::Node,
    fn_name: &str,
    receiver_type: Option<&str>,
    src: &[u8],
) -> Option<NodeRole> {
    classify_structural(rules, node)
        .or_else(|| classify_contextual(rules, node, fn_name, receiver_type, src))
}

fn classify_structural(rules: &dyn LanguageRules, node: tree_sitter::Node) -> Option<NodeRole> {
    if let Some(construct) = rules.flow_construct(node) {
        return Some(NodeRole::FlowConstruct(construct));
    }
    if rules.is_else_clause(node) {
        return Some(NodeRole::ElseClause);
    }
    if rules.nesting_kind(node).is_some() {
        return Some(NodeRole::NestingBoundary);
    }
    if rules.logical_operator(node).is_some() {
        return Some(NodeRole::LogicalExpression);
    }
    None
}

fn classify_contextual(
    rules: &dyn LanguageRules,
    node: tree_sitter::Node,
    fn_name: &str,
    receiver_type: Option<&str>,
    src: &[u8],
) -> Option<NodeRole> {
    if let Some((keyword, label)) = rules.jump_label(node, src) {
        return Some(NodeRole::LabeledJump(keyword, label));
    }
    if rules.is_recursive_call(node, fn_name, receiver_type, src) {
        return Some(NodeRole::RecursiveCall);
    }
    None
}

struct ScoringContext<'a> {
    rules: &'a dyn LanguageRules,
    fn_name: &'a str,
    impl_type: Option<&'a str>,
    src: &'a [u8],
    contributors: &'a mut Vec<Contributor>,
}

impl ScoringContext<'_> {
    fn complexity(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .map(|child| self.score_node(child, nesting))
            .sum()
    }

    fn score_node(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        let Some(role) = classify(self.rules, node, self.fn_name, self.impl_type, self.src) else {
            return self.complexity(node, nesting);
        };
        match role {
            NodeRole::FlowConstruct(construct) => self.score_flow_break(node, nesting, construct),
            NodeRole::ElseClause => self.score_else(node, nesting),
            NodeRole::NestingBoundary => self.complexity(node, nesting + 1),
            NodeRole::LogicalExpression => self.score_logical_sequence(node, nesting),
            NodeRole::LabeledJump(keyword, label) => self.score_jump(node, nesting, keyword, label),
            NodeRole::RecursiveCall => self.score_recursion(node, nesting),
        }
    }

    fn push(&mut self, kind: ContributorKind, node: tree_sitter::Node, increment: u64) {
        self.contributors.push(Contributor {
            kind,
            line: node.start_position().row + 1,
            increment,
        });
    }

    fn score_flow_break(
        &mut self,
        node: tree_sitter::Node,
        nesting: u64,
        construct: FlowConstruct,
    ) -> u64 {
        let increment = 1 + nesting;
        let kind = if nesting > 0 {
            let mut chain = nesting_chain(node, self.rules);
            chain.push(construct);
            ContributorKind::Nesting {
                construct,
                depth: nesting,
                chain,
            }
        } else {
            ContributorKind::FlowBreak { construct }
        };
        self.push(kind, node, increment);
        increment + self.complexity(node, nesting + 1)
    }

    fn score_else(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        if self.rules.is_else_if(node) {
            self.complexity(node, nesting.saturating_sub(1))
        } else {
            self.push(ContributorKind::Else, node, 1);
            1 + self.complexity(node, nesting)
        }
    }

    fn score_jump(
        &mut self,
        node: tree_sitter::Node,
        nesting: u64,
        keyword: JumpKeyword,
        label: String,
    ) -> u64 {
        self.push(ContributorKind::Jump { keyword, label }, node, 1);
        1 + self.complexity(node, nesting)
    }

    fn score_recursion(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        self.push(
            ContributorKind::Recursion {
                fn_name: self.fn_name.to_string(),
            },
            node,
            1,
        );
        1 + self.complexity(node, nesting)
    }

    fn score_logical_sequence(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        let mut operators = vec![];
        collect_logical_operators(node, self.rules, &mut operators);

        let score = count_operator_sequences(&operators);
        if score > 0 {
            self.push(ContributorKind::Logical { operators }, node, score);
        }

        score + self.visit_logical_leaves(node, nesting)
    }

    fn visit_logical_leaves(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        let mut cursor = node.walk();
        let children: Vec<_> = node
            .children(&mut cursor)
            .filter(|child| !self.rules.is_logical_operator_token(*child))
            .collect();

        children
            .into_iter()
            .map(|child| {
                if self.rules.logical_operator(child).is_some() {
                    self.visit_logical_leaves(child, nesting)
                } else {
                    self.complexity(child, nesting)
                }
            })
            .sum()
    }
}

fn nesting_chain(node: tree_sitter::Node, rules: &dyn LanguageRules) -> Vec<FlowConstruct> {
    let ancestors = std::iter::successors(node.parent(), tree_sitter::Node::parent);
    let mut chain = Vec::new();
    for ancestor in ancestors {
        if let Some(construct) = rules.flow_construct(ancestor) {
            chain.push(construct);
            continue;
        }
        match rules.nesting_kind(ancestor) {
            Some(NestingKind::Inline(construct)) => {
                chain.push(construct);
                break;
            }
            Some(NestingKind::Separate) => break,
            None => {}
        }
    }
    chain.reverse();
    chain
}

fn count_operator_sequences(operators: &[LogicalOp]) -> u64 {
    operators
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .count() as u64
        + u64::from(!operators.is_empty())
}

fn collect_logical_operators(
    node: tree_sitter::Node,
    rules: &dyn LanguageRules,
    operators: &mut Vec<LogicalOp>,
) {
    let Some(op) = rules.logical_operator(node) else {
        return;
    };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if rules.logical_operator(child).is_some() {
            collect_logical_operators(child, rules, operators);
        }
    }
    operators.push(op);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_complexity::rust::Rust;
    use test_case::test_case;

    // Source fixtures: named by scenario, not by language.
    // We use Rust as the AST vehicle, but tests express pure cognitive complexity rules.
    const FLAT_FUNCTION: &str = "fn f() { 1 + 2 }";
    const SINGLE_CONDITIONAL: &str = "fn f() { if true {} }";
    const CONDITIONAL_WITH_ELSE: &str = "fn f(x: bool) { if x {} else {} }";
    const LOOP_WITH_NESTED_CONDITIONAL: &str = "fn f() { for x in [1] { if true {} } }";
    const LOOP_WITH_INLINE_NESTING_AND_CONDITIONAL: &str =
        "fn f() { for x in [1] { [1].iter().filter(|y| { if **y > 0 {} }); } }";
    const MIXED_LOGICAL_OPERATORS: &str = "fn f(a: bool, b: bool, c: bool) -> bool { a && b || c }";
    const DIRECT_RECURSION: &str = "fn go(n: u64) -> u64 { go(n - 1) }";
    const LABELED_BREAK: &str = "fn f() { 'outer: loop { break 'outer; } }";
    const LABELED_CONTINUE: &str = "fn f() { 'outer: loop { continue 'outer; } }";
    const ELSE_IF_WITH_NESTED_CONDITIONAL: &str = "fn f(x: i32, y: bool) -> i32 {
        if x > 0 { 1 }
        else if y { if x < -10 { 2 } else { 3 } }
        else { 0 }
    }";
    // loop > conditional > conditional + else = 1 + (1+1) + (1+2) + 1
    const DEEPLY_NESTED: &str = "fn process(items: &[i32]) -> i32 {
        let mut total = 0;
        for item in items {
            if *item > 0 {
                if *item > 10 { total += item; }
                else { total -= item; }
            }
        }
        total
    }";
    const EMPTY_SOURCE: &str = "";
    const BROKEN_SYNTAX: &str = "fn f(x: i32 -> { x + }";

    fn rules() -> &'static dyn LanguageRules {
        &Rust
    }

    fn contributors(source: &str) -> Vec<Contributor> {
        let results = score_functions(source, rules());
        assert_eq!(results.len(), 1, "expected exactly one function");
        results.into_iter().next().unwrap().contributors
    }

    fn score(source: &str) -> u64 {
        let results = score_functions(source, rules());
        assert_eq!(results.len(), 1, "expected exactly one function");
        results[0].score
    }

    #[test]
    fn flat_function_has_no_contributors() {
        assert!(contributors(FLAT_FUNCTION).is_empty());
    }

    #[test]
    fn flow_break_increments_by_one_at_depth_zero() {
        let cs = contributors(SINGLE_CONDITIONAL);

        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].increment, 1);
        assert!(matches!(cs[0].kind, ContributorKind::FlowBreak { .. }));
    }

    #[test]
    fn nesting_increments_by_one_plus_depth() {
        let cs = contributors(LOOP_WITH_NESTED_CONDITIONAL);
        let nested = cs
            .iter()
            .find(|c| matches!(c.kind, ContributorKind::Nesting { .. }))
            .unwrap();

        assert_eq!(nested.increment, 2); // 1 + depth 1
        if let ContributorKind::Nesting { depth, chain, .. } = &nested.kind {
            assert_eq!(*depth, 1);
            assert_eq!(chain.len(), 2);
        }
    }

    #[test]
    fn else_increments_by_one() {
        let cs = contributors(CONDITIONAL_WITH_ELSE);
        let else_c = cs
            .iter()
            .find(|c| matches!(c.kind, ContributorKind::Else))
            .unwrap();

        assert_eq!(else_c.increment, 1);
    }

    #[test]
    fn logical_operators_count_sequence_changes() {
        let cs = contributors(MIXED_LOGICAL_OPERATORS);

        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].increment, 2); // && then || = 2 sequences
        if let ContributorKind::Logical { operators } = &cs[0].kind {
            assert_eq!(operators.len(), 2);
        } else {
            panic!("expected logical contributor");
        }
    }

    #[test]
    fn recursion_increments_by_one() {
        let cs = contributors(DIRECT_RECURSION);
        let rec = cs
            .iter()
            .find(|c| matches!(c.kind, ContributorKind::Recursion { .. }))
            .unwrap();

        assert_eq!(rec.increment, 1);
    }

    #[test_case(LABELED_BREAK, JumpKeyword::Break ; "labeled_break")]
    #[test_case(LABELED_CONTINUE, JumpKeyword::Continue ; "labeled_continue")]
    fn labeled_jump_increments_by_one(source: &str, expected_keyword: JumpKeyword) {
        let cs = contributors(source);
        let jump = cs
            .iter()
            .find(|c| matches!(c.kind, ContributorKind::Jump { .. }))
            .unwrap();

        assert_eq!(jump.increment, 1);
        if let ContributorKind::Jump { keyword, .. } = &jump.kind {
            assert_eq!(*keyword, expected_keyword);
        }
    }

    #[test]
    fn nesting_chain_includes_inline_boundary() {
        let cs = contributors(LOOP_WITH_INLINE_NESTING_AND_CONDITIONAL);
        let nested = cs
            .iter()
            .find(|c| matches!(c.kind, ContributorKind::Nesting { .. }))
            .unwrap();

        assert_eq!(nested.increment, 3); // 1 + depth 2
        if let ContributorKind::Nesting { depth, chain, .. } = &nested.kind {
            assert_eq!(*depth, 2);
            assert_eq!(chain[0].role, Construct::InlineNesting);
        }
    }

    #[test]
    fn else_if_does_not_increase_nesting_for_subsequent_branch() {
        assert_eq!(score(ELSE_IF_WITH_NESTED_CONDITIONAL), 6);
    }

    #[test]
    fn deeply_nested_function_accumulates_nesting_penalties() {
        assert_eq!(score(DEEPLY_NESTED), 7);
    }

    #[test]
    fn empty_source_returns_no_functions() {
        assert!(score_functions(EMPTY_SOURCE, rules()).is_empty());
    }

    #[test]
    fn broken_syntax_does_not_panic() {
        let _ = score_functions(BROKEN_SYNTAX, rules());
    }
}
