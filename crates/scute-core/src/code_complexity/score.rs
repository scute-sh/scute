use crate::files::SourceFile;
use crate::language::LanguageRegistry;
use crate::parser::{AstParser, TreeSitterParser};

use super::rules::{
    ComplexityRules, Construct, ElseBranch, FlowConstruct, IfConstruct, JumpKeyword, LogicalOp,
    NestingKind,
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
    LogicalExpression,
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
    collect_functions(tree.root_node(), src, rules, &mut results);
    results
}

fn collect_functions(
    node: tree_sitter::Node,
    src: &[u8],
    rules: &dyn ComplexityRules,
    results: &mut Vec<FunctionScore>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(unit) = rules.scoring_unit(child, src) {
            let mut contributors = vec![];
            let score = ScoringContext {
                rules,
                fn_name: &unit.name,
                receiver_type: unit.receiver_type.as_deref(),
                src,
                contributors: &mut contributors,
                enclosing: vec![],
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
    if rules.logical_operator(node).is_some() {
        return Some(NodeRole::LogicalExpression);
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
    /// The structures around the node being scored, outermost first, one per
    /// nesting level. Nesting evidence names them.
    enclosing: Vec<FlowConstruct>,
}

impl ScoringContext<'_> {
    fn complexity(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .map(|child| self.score_node(child, nesting))
            .sum()
    }

    fn score_node(&mut self, node: tree_sitter::Node, nesting: u64) -> u64 {
        let Some(role) = classify(self.rules, node, self.fn_name, self.receiver_type, self.src)
        else {
            return self.complexity(node, nesting);
        };
        match role {
            NodeRole::If(construct) => self.score_if(node, construct, nesting),
            NodeRole::FlowConstruct(construct) => self.score_flow_break(node, nesting, construct),
            NodeRole::NestingBoundary(kind) => self.score_nesting_boundary(node, nesting, kind),
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

    /// Records the `1 + nesting` increment of a structure and returns it.
    fn push_structural(
        &mut self,
        node: tree_sitter::Node,
        nesting: u64,
        construct: FlowConstruct,
    ) -> u64 {
        debug_assert_eq!(nesting, self.enclosing.len() as u64);
        let increment = 1 + nesting;
        let kind = if nesting > 0 {
            ContributorKind::Nesting {
                construct,
                enclosing: self.enclosing.clone(),
            }
        } else {
            ContributorKind::FlowBreak { construct }
        };
        self.push(kind, node, increment);
        increment
    }

    /// Scores with `construct` added to the enclosing structures.
    fn inside(&mut self, construct: FlowConstruct, score: impl FnOnce(&mut Self) -> u64) -> u64 {
        self.enclosing.push(construct);
        let total = score(self);
        self.enclosing.pop();
        total
    }

    fn score_flow_break(
        &mut self,
        node: tree_sitter::Node,
        nesting: u64,
        construct: FlowConstruct,
    ) -> u64 {
        let increment = self.push_structural(node, nesting, construct);
        increment + self.inside(construct, |ctx| ctx.complexity(node, nesting + 1))
    }

    fn score_if(&mut self, node: tree_sitter::Node, construct: IfConstruct, nesting: u64) -> u64 {
        let increment = self.push_structural(node, nesting, IF);
        increment + self.score_if_parts(construct, nesting, IF)
    }

    /// Scores an `if`'s condition at the `if`'s own level, then its consequence
    /// and `else` one level deeper. `branch` names the consequence in evidence.
    fn score_if_parts(
        &mut self,
        construct: IfConstruct,
        nesting: u64,
        branch: FlowConstruct,
    ) -> u64 {
        let condition = self.score_node(construct.condition, nesting);
        let consequence = self.inside(branch, |ctx| {
            ctx.score_node(construct.consequence, nesting + 1)
        });
        let alternative = construct
            .alternative
            .map_or(0, |alternative| self.score_else(alternative, nesting));
        condition + consequence + alternative
    }

    fn score_else(&mut self, branch: ElseBranch, nesting: u64) -> u64 {
        if let Some(chained) = self.rules.if_construct(branch.body) {
            self.push_hybrid(ELSE_IF, branch.keyword);
            return 1 + self.score_if_parts(chained, nesting, ELSE_IF);
        }
        self.push_hybrid(ELSE, branch.keyword);
        1 + self.inside(ELSE, |ctx| ctx.score_node(branch.body, nesting + 1))
    }

    fn push_hybrid(&mut self, construct: FlowConstruct, keyword: tree_sitter::Node) {
        let label = construct.label;
        self.push(ContributorKind::Else { label }, keyword, 1);
    }

    fn score_nesting_boundary(
        &mut self,
        node: tree_sitter::Node,
        nesting: u64,
        kind: NestingKind,
    ) -> u64 {
        self.inside(kind.construct(), |ctx| ctx.complexity(node, nesting + 1))
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

fn count_operator_sequences(operators: &[LogicalOp]) -> u64 {
    operators
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .count() as u64
        + u64::from(!operators.is_empty())
}

fn collect_logical_operators(
    node: tree_sitter::Node,
    rules: &dyn ComplexityRules,
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
