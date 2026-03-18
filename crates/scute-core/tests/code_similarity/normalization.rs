use std::path::Path;

use scute_core::code_similarity::rules::SimilarityRules;
use scute_core::code_similarity::{javascript::JsFamily, parse_source, rust::Rust};

#[test]
fn line_numbers_are_one_indexed() {
    let source = "\
fn foo() {
    let x = 1;
}";

    let tree = parse_source(source, "a.rs", Path::new("a.rs"), &Rust).unwrap();
    let tokens = tree.tokens();

    assert_eq!(tokens.first().unwrap().start_line, 1);
    assert_eq!(tokens.last().unwrap().end_line, 3);
}

fn normalized_texts(source: &str, file: &str, rules: &dyn SimilarityRules) -> String {
    let tree = parse_source(source, file, Path::new(file), rules).unwrap();
    tree.tokens()
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

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

    let tokens_a = normalized_texts(input_a, "a.rs", &Rust);
    let tokens_b = normalized_texts(input_b, "b.rs", &Rust);

    insta::assert_snapshot!("input_a", &tokens_a);
    insta::assert_snapshot!("input_b", &tokens_b);
    assert_eq!(
        tokens_a, tokens_b,
        "Type-2 clones should normalize to the same token sequence"
    );
}

macro_rules! snapshot_normalization {
    ($name:ident, $rules:expr, $file:expr, $source:expr) => {
        #[test]
        fn $name() {
            insta::assert_snapshot!(normalized_texts($source, $file, &$rules));
        }
    };
}

snapshot_normalization!(
    normalizes_rust_function,
    Rust,
    "a.rs",
    "\
fn add(a: i32, b: i32) -> i32 {
    a + b
}"
);

snapshot_normalization!(
    normalizes_typescript_with_literals_and_strips_comments,
    JsFamily::typescript(),
    "a.ts",
    "\
// helper function
const greet = (name: string) => {
  console.log(\"hello\", name);
  return 42;
}"
);

snapshot_normalization!(
    strips_rust_attributes,
    Rust,
    "a.rs",
    "\
#[derive(Debug, Clone)]
#[serde(rename_all = \"camelCase\")]
struct Foo {
    bar: String,
}"
);

snapshot_normalization!(
    strips_rust_inner_attributes,
    Rust,
    "a.rs",
    "\
#![allow(unused)]
fn main() {}"
);

snapshot_normalization!(
    strips_typescript_decorators,
    JsFamily::typescript(),
    "a.ts",
    "\
@Injectable()
@Component({ selector: 'app-root' })
class AppComponent {
  name: string = 'hello';
}"
);

snapshot_normalization!(
    strips_rust_doc_comments,
    Rust,
    "a.rs",
    "\
/// This is a doc comment.
///
/// With multiple lines.
fn documented() {}"
);

snapshot_normalization!(
    preserves_macro_invocations,
    Rust,
    "a.rs",
    "\
fn example() {
    let v = vec![1, 2, 3];
    assert_eq!(v.len(), 3);
}"
);
