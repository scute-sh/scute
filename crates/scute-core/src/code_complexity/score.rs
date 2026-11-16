use crate::files::SourceFile;
use crate::language::LanguageRegistry;
use crate::parser::{AstParser, TreeSitterParser};

use super::rules::{
    ComplexityRules, Construct, ElseBranch, FlowConstruct, IfConstruct, JumpKeyword,
    LogicalExpression, LogicalOp, NestingKind, ScoringUnit,
};

const IF: FlowConstruct = FlowConstruct {
    role: Construct::Conditional,
    label: "if",
};
const ELSE_IF: FlowConstruct = FlowConstruct {
    role: Construct::Conditional,
    label: "else if",
};
const ELSE: FlowConstruct = FlowConstruct {
    role: Construct::Conditional,
    label: "else",
};

enum NodeRole<'t> {
    If(IfConstruct<'t>),
    FlowConstruct(FlowConstruct),
    NestingBoundary(NestingKind),
    LogicalExpression(LogicalExpression<'t>),
    LabeledJump(JumpKeyword, String),
    RecursiveCall,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContributorKind {
    FlowBreak {
        construct: FlowConstruct,
    },
    /// A structure nested one level deep per entry in `enclosing`, which lists
    /// the structures around it, outermost first.
    Nesting {
        construct: FlowConstruct,
        enclosing: Vec<FlowConstruct>,
    },
    Else {
        label: &'static str,
    },
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

#[derive(Debug)]
pub struct FunctionScore {
    pub name: String,
    pub line: usize,
    pub score: u64,
    pub contributors: Vec<Contributor>,
}

pub fn score_functions(
    file: &SourceFile,
    languages: &LanguageRegistry<dyn ComplexityRules>,
) -> Vec<FunctionScore> {
    let Some((lang, rules)) = languages.for_path(&file.path) else {
        return vec![];
    };
    let mut parser = TreeSitterParser::new();
    let Ok(tree) = parser.parse(&file.content, &lang.grammar) else {
        return vec![];
    };
    let src = file.content.as_bytes();
    let mut results = vec![];
    collect_functions(tree.root_node(), src, rules, true, &mut results);
    results
}

/// Scores every function under `node`. `outer` holds while no function
/// encloses `node`.
fn collect_functions(
    node: tree_sitter::Node,
    src: &[u8],
    rules: &dyn ComplexityRules,
    outer: bool,
    results: &mut Vec<FunctionScore>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let unit = rules.scoring_unit(child, src);
        let child_is_outer = outer && unit.is_none() && rules.nesting_kind(child).is_none();
        if let Some(unit) = unit {
            results.push(score_unit(unit, outer, src, rules));
        }
        collect_functions(child, src, rules, child_is_outer, results);
    }
}

fn score_unit(
    unit: ScoringUnit,
    outer: bool,
    src: &[u8],
    rules: &dyn ComplexityRules,
) -> FunctionScore {
    let mut contributors = vec![];
    let score = ScoringContext {
        rules,
        fn_name: &unit.name,
        receiver_type: unit.receiver_type.as_deref(),
        src,
        contributors: &mut contributors,
        enclosing: vec![],
        declarative_top_level: outer
            && rules.ignores_declarative_outer_functions()
            && !has_top_level_structure(rules, unit.body),
    }
    .score_node(unit.body);
    FunctionScore {
        name: unit.name,
        line: unit.line,
        score,
        contributors,
    }
}

/// Whether a structural increment sits under `node` outside any function
/// nested in it.
fn has_top_level_structure(rules: &dyn ComplexityRules, node: tree_sitter::Node) -> bool {
    match classify_structural(rules, node) {
        Some(NodeRole::If(_) | NodeRole::FlowConstruct(_)) => true,
        Some(NodeRole::NestingBoundary(_)) => false,
        _ => node
            .children(&mut node.walk())
            .any(|child| has_top_level_structure(rules, child)),
    }
}

fn classify<'t>(
    rules: &dyn ComplexityRules,
    node: tree_sitter::Node<'t>,
    fn_name: &str,
    receiver_type: Option<&str>,
    src: &[u8],
) -> Option<NodeRole<'t>> {
    classify_structural(rules, node)
        .or_else(|| classify_contextual(rules, node, fn_name, receiver_type, src))
}

fn classify_structural<'t>(
    rules: &dyn ComplexityRules,
    node: tree_sitter::Node<'t>,
) -> Option<NodeRole<'t>> {
    if let Some(construct) = rules.if_construct(node) {
        return Some(NodeRole::If(construct));
    }
    if let Some(construct) = rules.flow_construct(node) {
        return Some(NodeRole::FlowConstruct(construct));
    }
    if let Some(kind) = rules.nesting_kind(node) {
        return Some(NodeRole::NestingBoundary(kind));
    }
    if let Some(expression) = rules.logical_expression(node) {
        return Some(NodeRole::LogicalExpression(expression));
    }
    None
}

fn classify_contextual<'t>(
    rules: &dyn ComplexityRules,
    node: tree_sitter::Node,
    fn_name: &str,
    receiver_type: Option<&str>,
    src: &[u8],
) -> Option<NodeRole<'t>> {
    if let Some((keyword, label)) = rules.jump_label(node, src) {
        return Some(NodeRole::LabeledJump(keyword, label));
    }
    if rules.is_recursive_call(node, fn_name, receiver_type, src) {
        return Some(NodeRole::RecursiveCall);
    }
    None
}

struct ScoringContext<'a> {
    rules: &'a dyn ComplexityRules,
    fn_name: &'a str,
    receiver_type: Option<&'a str>,
    src: &'a [u8],
    contributors: &'a mut Vec<Contributor>,
    /// The structures around the node being scored, outermost first. Each one
    /// is a nesting level: increments count them and evidence names them.
    enclosing: Vec<FlowConstruct>,
    /// Whether the node being scored sits at the top level of a declarative
    /// outer function, where nested functions add no nesting.
    declarative_top_level: bool,
}

impl ScoringContext<'_> {
    fn complexity(&mut self, node: tree_sitter::Node) -> u64 {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .map(|child| self.score_node(child))
            .sum()
    }

    fn score_node(&mut self, node: tree_sitter::Node) -> u64 {
        let Some(role) = classify(self.rules, node, self.fn_name, self.receiver_type, self.src)
        else {
            return self.complexity(node);
        };
        match role {
            NodeRole::If(construct) => self.score_if(node, construct),
            NodeRole::FlowConstruct(construct) => self.score_flow_break(node, construct),
            NodeRole::NestingBoundary(kind) => self.score_nesting_boundary(node, kind),
            NodeRole::LogicalExpression(expression) => {
                self.score_logical_sequence(node, expression)
            }
            NodeRole::LabeledJump(keyword, label) => self.score_jump(node, keyword, label),
            NodeRole::RecursiveCall => self.score_recursion(node),
        }
    }

    fn push(&mut self, kind: ContributorKind, node: tree_sitter::Node, increment: u64) {
        self.contributors.push(Contributor {
            kind,
            line: node.start_position().row + 1,
            increment,
        });
    }

    /// Records the `1 + nesting` increment of a structure and returns it.
    fn push_structural(&mut self, node: tree_sitter::Node, construct: FlowConstruct) -> u64 {
        let increment = 1 + self.enclosing.len() as u64;
        let kind = if self.enclosing.is_empty() {
            ContributorKind::FlowBreak { construct }
        } else {
            ContributorKind::Nesting {
                construct,
                enclosing: self.enclosing.clone(),
            }
        };
        self.push(kind, node, increment);
        increment
    }

    /// Scores one level deeper, with `construct` added to the enclosing
    /// structures.
    fn inside(&mut self, construct: FlowConstruct, score: impl FnOnce(&mut Self) -> u64) -> u64 {
        self.enclosing.push(construct);
        let total = score(self);
        self.enclosing.pop();
        total
    }

    fn score_flow_break(&mut self, node: tree_sitter::Node, construct: FlowConstruct) -> u64 {
        let increment = self.push_structural(node, construct);
        increment + self.inside(construct, |ctx| ctx.complexity(node))
    }

    fn score_if(&mut self, node: tree_sitter::Node, construct: IfConstruct) -> u64 {
        let increment = self.push_structural(node, IF);
        increment + self.score_if_parts(construct, IF)
    }

    /// Scores an `if`'s condition at the `if`'s own level, then its consequence
    /// and `else` one level deeper. `branch` names the consequence in evidence.
    fn score_if_parts(&mut self, construct: IfConstruct, branch: FlowConstruct) -> u64 {
        let condition = self.score_node(construct.condition);
        let consequence = self.inside(branch, |ctx| ctx.score_node(construct.consequence));
        let alternative = construct
            .alternative
            .map_or(0, |alternative| self.score_else(alternative));
        condition + consequence + alternative
    }

    fn score_else(&mut self, branch: ElseBranch) -> u64 {
        if let Some(chained) = self.rules.if_construct(branch.body) {
            self.push_hybrid(ELSE_IF, branch.keyword);
            return 1 + self.score_if_parts(chained, ELSE_IF);
        }
        self.push_hybrid(ELSE, branch.keyword);
        1 + self.inside(ELSE, |ctx| ctx.score_node(branch.body))
    }

    fn push_hybrid(&mut self, construct: FlowConstruct, keyword: tree_sitter::Node) {
        let label = construct.label;
        self.push(ContributorKind::Else { label }, keyword, 1);
    }

    /// A function at the top level of a declarative outer function adds no
    /// level, but the functions nested in it do.
    fn score_nesting_boundary(&mut self, node: tree_sitter::Node, kind: NestingKind) -> u64 {
        if !self.declarative_top_level {
            return self.inside(kind.construct(), |ctx| ctx.complexity(node));
        }
        self.declarative_top_level = false;
        let total = self.complexity(node);
        self.declarative_top_level = true;
        total
    }

    fn score_jump(&mut self, node: tree_sitter::Node, keyword: JumpKeyword, label: String) -> u64 {
        self.push(ContributorKind::Jump { keyword, label }, node, 1);
        1 + self.complexity(node)
    }

    fn score_recursion(&mut self, node: tree_sitter::Node) -> u64 {
        self.push(
            ContributorKind::Recursion {
                fn_name: self.fn_name.to_string(),
            },
            node,
            1,
        );
        1 + self.complexity(node)
    }

    fn score_logical_sequence(
        &mut self,
        node: tree_sitter::Node,
        expression: LogicalExpression,
    ) -> u64 {
        let mut flat = FlatLogicalExpression::default();
        flat.flatten(self.rules, expression);

        let score = count_operator_sequences(&flat.operators);
        if score > 0 {
            let operators = flat.operators;
            self.push(ContributorKind::Logical { operators }, node, score);
        }

        score
            + flat
                .operands
                .into_iter()
                .map(|operand| self.complexity(operand))
                .sum::<u64>()
    }
}

/// A logical expression read left to right as one run of operators and the
/// operands between them, looking through parentheses.
#[derive(Default)]
struct FlatLogicalExpression<'t> {
    operators: Vec<LogicalOp>,
    operands: Vec<tree_sitter::Node<'t>>,
}

impl<'t> FlatLogicalExpression<'t> {
    fn flatten(&mut self, rules: &dyn ComplexityRules, expression: LogicalExpression<'t>) {
        self.read_operand(rules, expression.left);
        self.operators.push(expression.operator);
        self.read_operand(rules, expression.right);
    }

    fn read_operand(&mut self, rules: &dyn ComplexityRules, operand: tree_sitter::Node<'t>) {
        match rules.logical_expression(operand) {
            Some(expression) => self.flatten(rules, expression),
            None => self.operands.push(operand),
        }
    }
}

fn count_operator_sequences(operators: &[LogicalOp]) -> u64 {
    operators
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .count() as u64
        + u64::from(!operators.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_complexity::check::languages;
    use crate::code_complexity::rules::Construct;
    use crate::files::SourceFile;
    use googletest::prelude::*;
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

    fn file(source: &str) -> SourceFile {
        SourceFile {
            path: "test.rs".into(),
            content: source.into(),
        }
    }

    fn contributors(source: &str) -> Vec<Contributor> {
        let results = score_functions(&file(source), &languages());
        assert_eq!(results.len(), 1, "expected exactly one function");
        results.into_iter().next().unwrap().contributors
    }

    fn score(source: &str) -> u64 {
        let results = score_functions(&file(source), &languages());
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
        if let ContributorKind::Nesting { enclosing, .. } = &nested.kind {
            assert_eq!(enclosing.len(), 1);
        }
    }

    #[test]
    fn else_increments_by_one() {
        let cs = contributors(CONDITIONAL_WITH_ELSE);
        let else_c = cs
            .iter()
            .find(|c| matches!(c.kind, ContributorKind::Else { .. }))
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
    fn nesting_chain_runs_through_inline_boundary() {
        let cs = contributors(LOOP_WITH_INLINE_NESTING_AND_CONDITIONAL);
        let nested = cs
            .iter()
            .find(|c| matches!(c.kind, ContributorKind::Nesting { .. }))
            .unwrap();

        assert_eq!(nested.increment, 3); // 1 + depth 2
        let ContributorKind::Nesting { enclosing, .. } = &nested.kind else {
            panic!("expected nesting contributor");
        };
        let roles: Vec<_> = enclosing.iter().map(|c| c.role).collect();
        assert_eq!(roles, [Construct::Loop, Construct::NestedFunction]);
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
        assert!(score_functions(&file(""), &languages()).is_empty());
    }

    #[test]
    fn scores_well_formed_function_next_to_broken_one() {
        let source = "fn valid() { if true {} }\nfn broken(x: i32 -> { x + }";

        let results = score_functions(&file(source), &languages());

        assert_that!(
            results,
            contains(matches_pattern!(FunctionScore {
                name: eq("valid"),
                score: eq(&1),
                ..
            }))
        );
    }
}
