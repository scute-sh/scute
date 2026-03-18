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
        name: String,
    },
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
    /// # Panics (debug)
    ///
    /// Debug-asserts that all containers have been closed (only the root
    /// Source node remains on the stack).
    #[must_use]
    pub fn build(self) -> SourceTree {
        debug_assert!(
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
    pub node_index: usize,
    pub text: String,
    pub start_line: usize,
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

    /// Walk up from a node to find the enclosing Contract name, if any.
    #[must_use]
    pub fn enclosing_contract(&self, node_index: usize) -> Option<&str> {
        let mut idx = node_index;
        loop {
            match &self.nodes[idx].kind {
                NodeKind::Contract { name } => return Some(name),
                _ => idx = self.nodes[idx].parent?,
            }
        }
    }

    /// Walk up from a node to check if it's inside a `TestRegion`.
    #[must_use]
    pub fn is_in_test_region(&self, node_index: usize) -> bool {
        let mut idx = node_index;
        loop {
            if matches!(&self.nodes[idx].kind, NodeKind::TestRegion) {
                return true;
            }
            let Some(parent) = self.nodes[idx].parent else {
                return false;
            };
            idx = parent;
        }
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
            name: "Render".to_string(),
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
            name: "Render".to_string(),
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
}
