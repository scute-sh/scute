use std::collections::BTreeSet;

/// A language-agnostic source tree for code similarity analysis.
///
/// Arena-based: nodes live in a flat `Vec`, linked by indices. Tokens are
/// leaves, context nodes (`Source`, `TestRegion`, `Contract`) are containers.
/// Each node has a parent index for walk-up queries.

#[derive(Debug)]
pub struct SourceTree {
    nodes: Vec<Node>,
}

#[derive(Debug)]
struct Node {
    kind: NodeKind,
    parent: Option<usize>,
    children: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Source {
        path: String,
    },
    TestRegion,
    Contract {
        names: Vec<String>,
    },
    Collection,
    Token {
        text: String,
        start_line: usize,
        end_line: usize,
    },
    Comment,
    Decoration,
}

/// Builds a `SourceTree` by tracking the current container on a stack.
pub struct SourceTreeBuilder {
    nodes: Vec<Node>,
    /// Stack of container indices. Top = current parent for new nodes.
    stack: Vec<usize>,
}

impl SourceTreeBuilder {
    /// Start building a tree rooted at a Source node.
    #[must_use]
    pub fn new(path: String) -> Self {
        let root = Node {
            kind: NodeKind::Source { path },
            parent: None,
            children: vec![],
        };
        Self {
            nodes: vec![root],
            stack: vec![0],
        }
    }

    /// Push a container node as a child of the current container.
    /// All subsequent tokens/containers are added under this one
    /// until `close_container()` is called.
    pub fn open_container(&mut self, kind: NodeKind) {
        let parent = self.current_parent();
        let idx = self.add_node(kind, parent);
        self.stack.push(idx);
    }

    /// Pop the current container, returning to the parent.
    ///
    /// # Panics
    ///
    /// Panics if called when only the root Source node remains on the stack.
    pub fn close_container(&mut self) {
        assert!(self.stack.len() > 1, "cannot pop the root node");
        self.stack.pop();
    }

    /// Add a token under the current container.
    pub fn add_token(&mut self, text: String, start_line: usize, end_line: usize) {
        let parent = self.current_parent();
        self.add_node(
            NodeKind::Token {
                text,
                start_line,
                end_line,
            },
            parent,
        );
    }

    /// Consume the builder and produce a `SourceTree`.
    ///
    /// # Panics
    ///
    /// Panics if not all containers have been closed (only the root
    /// Source node should remain on the stack).
    #[must_use]
    pub fn build(self) -> SourceTree {
        assert!(
            self.stack.len() == 1,
            "unclosed containers: expected stack depth 1, got {}",
            self.stack.len()
        );
        SourceTree { nodes: self.nodes }
    }

    fn current_parent(&self) -> usize {
        *self.stack.last().expect("stack is never empty")
    }

    fn add_node(&mut self, kind: NodeKind, parent: usize) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(Node {
            kind,
            parent: Some(parent),
            children: vec![],
        });
        self.nodes[parent].children.push(idx);
        idx
    }
}

/// A token extracted from the source tree, carrying its arena index for
/// walk-up queries.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// Index into the parent `SourceTree`'s arena. Used for walk-up queries
    /// (e.g. `is_in_test_region`, `ancestor_contracts`). Only meaningful
    /// within the tree that produced this token.
    pub node_index: usize,
    /// Normalized text (e.g. `$ID` for identifiers, `$LIT` for literals).
    pub text: String,
    /// 1-indexed start line in the original source.
    pub start_line: usize,
    /// 1-indexed end line in the original source.
    pub end_line: usize,
}

impl SourceTree {
    /// Returns the source path from the root Source node.
    ///
    /// # Panics
    ///
    /// Panics if the root node is not a `Source` node.
    #[must_use]
    pub fn source_id(&self) -> &str {
        match &self.nodes[0].kind {
            NodeKind::Source { path } => path,
            _ => panic!("root node must be Source"),
        }
    }

    /// Collect all tokens in document order.
    #[must_use]
    pub fn tokens(&self) -> Vec<Token> {
        let mut result = Vec::new();
        self.collect_tokens(0, &mut result);
        result
    }

    fn ancestor_contracts(&self, node_index: usize) -> impl Iterator<Item = &str> {
        self.ancestors(node_index)
            .filter_map(|kind| match kind {
                NodeKind::Contract { names } => Some(names),
                _ => None,
            })
            .flat_map(|names| names.iter().map(String::as_str))
    }

    /// Walk up from a node to check if it's inside a `TestRegion`.
    #[must_use]
    pub fn is_in_test_region(&self, node_index: usize) -> bool {
        self.ancestors(node_index)
            .any(|kind| matches!(kind, NodeKind::TestRegion))
    }

    /// Walk up from a node to check if it's inside a `Collection`.
    #[must_use]
    pub fn is_in_collection(&self, node_index: usize) -> bool {
        self.ancestors(node_index)
            .any(|kind| matches!(kind, NodeKind::Collection))
    }

    fn ancestors(&self, node_index: usize) -> impl Iterator<Item = &NodeKind> {
        let mut idx = Some(node_index);
        std::iter::from_fn(move || {
            let i = idx?;
            let node = &self.nodes[i];
            idx = node.parent;
            Some(&node.kind)
        })
    }

    fn collect_tokens(&self, idx: usize, result: &mut Vec<Token>) {
        let node = &self.nodes[idx];
        match &node.kind {
            NodeKind::Token {
                text,
                start_line,
                end_line,
            } => result.push(Token {
                node_index: idx,
                text: text.clone(),
                start_line: *start_line,
                end_line: *end_line,
            }),
            _ => {
                for &child in &node.children {
                    self.collect_tokens(child, result);
                }
            }
        }
    }
}

/// Returns true if all (tree, tokens) pairs share at least one common contract.
///
/// Each pair's tokens must all be enclosed by the same contract(s).
/// Then across pairs, there must be at least one contract name in common.
/// Returns false if any pair has tokens outside a contract.
#[must_use]
pub fn all_share_contract(pairs: &[(&SourceTree, &[Token])]) -> bool {
    let mut common: Option<BTreeSet<&str>> = None;
    for &(tree, tokens) in pairs {
        let contracts = contracts_enclosing(tree, tokens);
        if contracts.is_empty() {
            return false;
        }
        common = Some(match common {
            None => contracts,
            Some(prev) => prev.intersection(&contracts).copied().collect(),
        });
    }
    common.is_some_and(|c| !c.is_empty())
}

/// Returns the contracts that enclose ALL given tokens within a single tree.
fn contracts_enclosing<'a>(tree: &'a SourceTree, tokens: &[Token]) -> BTreeSet<&'a str> {
    let mut result: Option<BTreeSet<&str>> = None;
    for tok in tokens {
        let tok_contracts: BTreeSet<&str> = tree.ancestor_contracts(tok.node_index).collect();
        if tok_contracts.is_empty() {
            return BTreeSet::new();
        }
        result = Some(match result {
            None => tok_contracts,
            Some(prev) => prev.intersection(&tok_contracts).copied().collect(),
        });
    }
    result.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_with_tokens(path: &str, tokens: &[(&str, usize)]) -> SourceTree {
        let mut b = SourceTreeBuilder::new(path.to_string());
        for &(text, line) in tokens {
            b.add_token(text.to_string(), line, line);
        }
        b.build()
    }

    #[test]
    fn tokens_collects_in_insertion_order() {
        let tree = source_with_tokens("a.rs", &[("fn", 1), ("$ID", 1), ("(", 1)]);

        let tokens = tree.tokens();
        let texts: Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();

        assert_eq!(texts, vec!["fn", "$ID", "("]);
    }

    #[test]
    fn tokens_collects_across_containers() {
        let mut b = SourceTreeBuilder::new("a.rs".to_string());
        b.add_token("fn".to_string(), 1, 1);
        b.open_container(NodeKind::Contract {
            names: vec!["Render".to_string()],
        });
        b.add_token("impl".to_string(), 2, 2);
        b.close_container();
        b.add_token("let".to_string(), 5, 5);
        let tree = b.build();

        let tokens = tree.tokens();
        let texts: Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();

        assert_eq!(texts, vec!["fn", "impl", "let"]);
    }

    #[test]
    fn empty_tree_produces_no_tokens() {
        let tree = SourceTreeBuilder::new("empty.rs".to_string()).build();

        assert!(tree.tokens().is_empty());
    }

    #[test]
    fn tokens_carry_distinct_node_indices() {
        let tree = source_with_tokens("a.rs", &[("x", 1), ("y", 2)]);

        let tokens = tree.tokens();

        assert_ne!(tokens[0].node_index, tokens[1].node_index);
    }

    #[test]
    fn nested_containers_preserve_token_order() {
        let mut b = SourceTreeBuilder::new("a.rs".to_string());
        b.open_container(NodeKind::TestRegion);
        b.open_container(NodeKind::Contract {
            names: vec!["Render".to_string()],
        });
        b.add_token("fn".to_string(), 1, 1);
        b.add_token("$ID".to_string(), 1, 1);
        b.close_container();
        b.close_container();
        let tree = b.build();

        let tokens = tree.tokens();
        let texts: Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();

        assert_eq!(texts, vec!["fn", "$ID"]);
    }

    #[test]
    #[should_panic(expected = "unclosed containers")]
    fn build_panics_on_unclosed_container() {
        let mut b = SourceTreeBuilder::new("a.rs".to_string());
        b.open_container(NodeKind::TestRegion);
        b.add_token("fn".to_string(), 1, 1);
        let _tree = b.build();
    }

    fn contract_tree(path: &str, contracts: &[&str], token_texts: &[&str]) -> SourceTree {
        let mut b = SourceTreeBuilder::new(path.to_string());
        b.open_container(NodeKind::Contract {
            names: contracts.iter().map(ToString::to_string).collect(),
        });
        for (i, text) in token_texts.iter().enumerate() {
            b.add_token(text.to_string(), i + 1, i + 1);
        }
        b.close_container();
        b.build()
    }

    #[test]
    fn is_in_collection_true_for_tokens_inside_collection() {
        let mut b = SourceTreeBuilder::new("a.ts".to_string());
        b.open_container(NodeKind::Collection);
        b.add_token("$LIT".to_string(), 1, 1);
        b.add_token(",".to_string(), 1, 1);
        b.add_token("$LIT".to_string(), 1, 1);
        b.close_container();
        let tree = b.build();

        let tokens = tree.tokens();

        assert!(tokens.iter().all(|t| tree.is_in_collection(t.node_index)));
    }

    #[test]
    fn is_in_collection_false_for_tokens_outside_collection() {
        let tree = source_with_tokens("a.ts", &[("fn", 1), ("$ID", 1)]);

        let tokens = tree.tokens();

        assert!(tokens.iter().all(|t| !tree.is_in_collection(t.node_index)));
    }

    #[test]
    fn all_share_contract_true_when_same_contract_across_trees() {
        let tree_a = contract_tree("a.rs", &["Render"], &["fn", "$ID"]);
        let tree_b = contract_tree("b.rs", &["Render"], &["fn", "$ID"]);
        let tokens_a = tree_a.tokens();
        let tokens_b = tree_b.tokens();

        assert!(all_share_contract(&[
            (&tree_a, &tokens_a),
            (&tree_b, &tokens_b),
        ]));
    }

    #[test]
    fn all_share_contract_false_when_different_contracts() {
        let tree_a = contract_tree("a.rs", &["Render"], &["fn", "$ID"]);
        let tree_b = contract_tree("b.rs", &["Format"], &["fn", "$ID"]);
        let tokens_a = tree_a.tokens();
        let tokens_b = tree_b.tokens();

        assert!(!all_share_contract(&[
            (&tree_a, &tokens_a),
            (&tree_b, &tokens_b),
        ]));
    }

    #[test]
    fn all_share_contract_false_when_no_contract() {
        let tree_a = contract_tree("a.rs", &["Render"], &["fn", "$ID"]);
        let tree_b = source_with_tokens("b.rs", &[("fn", 1), ("$ID", 1)]);
        let tokens_a = tree_a.tokens();
        let tokens_b = tree_b.tokens();

        assert!(!all_share_contract(&[
            (&tree_a, &tokens_a),
            (&tree_b, &tokens_b),
        ]));
    }

    #[test]
    fn all_share_contract_true_when_overlapping_multi_contract() {
        let tree_a = contract_tree("a.ts", &["Renderer", "Base"], &["fn", "$ID"]);
        let tree_b = contract_tree("b.ts", &["Formatter", "Base"], &["fn", "$ID"]);
        let tokens_a = tree_a.tokens();
        let tokens_b = tree_b.tokens();

        assert!(all_share_contract(&[
            (&tree_a, &tokens_a),
            (&tree_b, &tokens_b),
        ]));
    }

    #[test]
    fn all_share_contract_false_for_empty_pairs() {
        assert!(!all_share_contract(&[]));
    }

    #[test]
    fn all_share_contract_false_when_no_overlap_in_multi_contract() {
        let tree_a = contract_tree("a.ts", &["Renderer", "Serializable"], &["fn", "$ID"]);
        let tree_b = contract_tree("b.ts", &["Formatter", "Disposable"], &["fn", "$ID"]);
        let tokens_a = tree_a.tokens();
        let tokens_b = tree_b.tokens();

        assert!(!all_share_contract(&[
            (&tree_a, &tokens_a),
            (&tree_b, &tokens_b),
        ]));
    }
}
