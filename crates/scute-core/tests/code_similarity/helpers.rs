use std::path::Path;

use scute_core::code_similarity::{
    CloneGroup, Token, detect_clones, parse_source, rules::SimilarityRules, tree::SourceTree,
};

pub struct DetectionResult {
    pub trees: Vec<SourceTree>,
    pub tokens: Vec<Vec<Token>>,
    pub groups: Vec<CloneGroup>,
}

pub fn parse_and_detect(
    files: &[(&str, &str, &dyn SimilarityRules)],
    min_tokens: usize,
) -> DetectionResult {
    let mut trees = Vec::new();

    for (source, path, rules) in files {
        let tree = parse_source(source, path, Path::new(path), *rules).unwrap();
        trees.push(tree);
    }

    let tokens: Vec<Vec<Token>> = trees.iter().map(SourceTree::tokens).collect();
    let groups = detect_clones(&tokens, min_tokens);

    DetectionResult {
        trees,
        tokens,
        groups,
    }
}

pub fn snapshot(result: &DetectionResult) -> String {
    if result.groups.is_empty() {
        return "(no clones)".to_string();
    }
    result
        .groups
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let header = format!("Group {} ({} tokens):", i + 1, g.token_count);
            let occs: Vec<String> = g
                .occurrences
                .iter()
                .map(|o| {
                    let tokens = &result.tokens[o.source_idx];
                    let start_line = tokens[o.token_start].start_line;
                    let end_idx = (o.token_start + g.token_count - 1).min(tokens.len() - 1);
                    let end_line = tokens[end_idx].end_line;
                    let source_id = result.trees[o.source_idx].source_id();
                    format!("  {source_id}:{start_line}-{end_line}")
                })
                .collect();
            format!("{header}\n{}", occs.join("\n"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
