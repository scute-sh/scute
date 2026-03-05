use scute_core::code_similarity::{CloneGroup, SourceTokens, language, tokenize};

pub fn tokenize_rust(source: &str, source_id: &str) -> SourceTokens {
    let tokens = tokenize(source, &language::rust()).unwrap();
    SourceTokens::new(source_id.to_string(), tokens)
}

pub fn snapshot(groups: &[CloneGroup]) -> String {
    if groups.is_empty() {
        return "(no clones)".to_string();
    }
    groups
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let header = format!("Group {} ({} tokens):", i + 1, g.token_count);
            let occs: Vec<String> = g
                .occurrences
                .iter()
                .map(|o| format!("  {}:{}-{}", o.source_id, o.start_line, o.end_line))
                .collect();
            format!("{header}\n{}", occs.join("\n"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
