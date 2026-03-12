use scute_core::code_similarity::{language, tokenize};
use scute_core::parser::TreeSitterParser;

#[test]
fn line_numbers_are_one_indexed() {
    let source = "\
fn foo() {
    let x = 1;
}";

    let mut parser = TreeSitterParser::new();
    let tokens = tokenize(&mut parser, source, &language::rust()).unwrap();

    assert_eq!(tokens.first().unwrap().start_line, 1);
    assert_eq!(tokens.last().unwrap().end_line, 3);
}

fn token_labels(source: &str, lang: &scute_core::code_similarity::LanguageConfig) -> String {
    let mut parser = TreeSitterParser::new();
    tokenize_to_labels(&mut parser, source, lang)
}

fn tokenize_to_labels(
    parser: &mut dyn scute_core::parser::AstParser,
    source: &str,
    lang: &scute_core::code_similarity::LanguageConfig,
) -> String {
    let tokens = tokenize(parser, source, lang).unwrap();
    tokens
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn reuses_parser_across_languages() {
    let mut parser = TreeSitterParser::new();

    let rust_tokens = tokenize_to_labels(
        &mut parser,
        "fn add(a: i32) -> i32 { a + 1 }",
        &language::rust(),
    );
    let ts_tokens = tokenize_to_labels(
        &mut parser,
        "function add(a: number): number { return a + 1; }",
        &language::typescript(),
    );
    let rust_after_ts = tokenize_to_labels(
        &mut parser,
        "fn add(b: u32) -> u32 { b + 1 }",
        &language::rust(),
    );

    // Same structure, different names/types → same normalized tokens
    assert_eq!(rust_tokens, rust_after_ts);
    // TS has different syntax (return keyword, semicolons)
    assert_ne!(rust_tokens, ts_tokens);
}

macro_rules! snapshot_tokenizer {
    ($name:ident, $lang:expr, $source:expr) => {
        #[test]
        fn $name() {
            insta::assert_snapshot!(token_labels($source, &$lang));
        }
    };
}

snapshot_tokenizer!(
    normalizes_rust_function,
    language::rust(),
    "\
fn add(a: i32, b: i32) -> i32 {
    a + b
}"
);

snapshot_tokenizer!(
    normalizes_typescript_with_literals_and_strips_comments,
    language::typescript(),
    "\
// helper function
const greet = (name: string) => {
  console.log(\"hello\", name);
  return 42;
}"
);

#[test]
fn renamed_identifiers_and_literals_produce_identical_tokens() {
    let input_a = "\
fn calculate(x: f64, y: f64) -> f64 {
    let result = process(x, y, \"multiply\");
    if result.success {
        return result.value;
    } else {
        return 0.0;
    }
}";

    let input_b = "\
fn transform(a: u32, b: u32) -> u32 {
    let output = convert(a, b, \"divide\");
    if output.success {
        return output.value;
    } else {
        return 99.9;
    }
}";

    let tokens_a = token_labels(input_a, &language::rust());
    let tokens_b = token_labels(input_b, &language::rust());

    insta::assert_snapshot!("input_a", &tokens_a);
    insta::assert_snapshot!("input_b", &tokens_b);
    assert_eq!(
        tokens_a, tokens_b,
        "Type-2 clones should normalize to the same token sequence"
    );
}

snapshot_tokenizer!(
    strips_rust_attributes,
    language::rust(),
    "\
#[derive(Debug, Clone)]
#[serde(rename_all = \"camelCase\")]
struct Foo {
    bar: String,
}"
);

snapshot_tokenizer!(
    strips_rust_inner_attributes,
    language::rust(),
    "\
#![allow(unused)]
fn main() {}"
);

snapshot_tokenizer!(
    strips_typescript_decorators,
    language::typescript(),
    "\
@Injectable()
@Component({ selector: 'app-root' })
class AppComponent {
  name: string = 'hello';
}"
);

snapshot_tokenizer!(
    strips_rust_doc_comments,
    language::rust(),
    "\
/// This is a doc comment.
///
/// With multiple lines.
fn documented() {}"
);

snapshot_tokenizer!(
    preserves_macro_invocations,
    language::rust(),
    "\
fn example() {
    let v = vec![1, 2, 3];
    assert_eq!(v.len(), 3);
}"
);
