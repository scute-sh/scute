use scute_core::code_similarity::{language, tokenize};

#[test]
fn line_numbers_are_one_indexed() {
    let source = "\
fn foo() {
    let x = 1;
}";

    let tokens = tokenize(source, &language::rust()).unwrap();

    assert_eq!(tokens.first().unwrap().start_line, 1);
    assert_eq!(tokens.last().unwrap().end_line, 3);
}

fn token_labels(source: &str, lang: &scute_core::code_similarity::LanguageConfig) -> String {
    let tokens = tokenize(source, lang).unwrap();
    tokens
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn normalizes_rust_function() {
    let source = "\
fn add(a: i32, b: i32) -> i32 {
    a + b
}";

    insta::assert_snapshot!(token_labels(source, &language::rust()));
}

#[test]
fn normalizes_typescript_with_literals_and_strips_comments() {
    let source = "\
// helper function
const greet = (name: string) => {
  console.log(\"hello\", name);
  return 42;
}";

    insta::assert_snapshot!(token_labels(source, &language::typescript()));
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

    let tokens_a = token_labels(input_a, &language::rust());
    let tokens_b = token_labels(input_b, &language::rust());

    insta::assert_snapshot!("input_a", &tokens_a);
    insta::assert_snapshot!("input_b", &tokens_b);
    assert_eq!(
        tokens_a, tokens_b,
        "Type-2 clones should normalize to the same token sequence"
    );
}

#[test]
fn strips_rust_attributes() {
    let source = "\
#[derive(Debug, Clone)]
#[serde(rename_all = \"camelCase\")]
struct Foo {
    bar: String,
}";

    insta::assert_snapshot!(token_labels(source, &language::rust()));
}

#[test]
fn strips_rust_inner_attributes() {
    let source = "\
#![allow(unused)]
fn main() {}";

    insta::assert_snapshot!(token_labels(source, &language::rust()));
}

#[test]
fn strips_typescript_decorators() {
    let source = "\
@Injectable()
@Component({ selector: 'app-root' })
class AppComponent {
  name: string = 'hello';
}";

    insta::assert_snapshot!(token_labels(source, &language::typescript()));
}

#[test]
fn preserves_macro_invocations() {
    let source = "\
fn example() {
    let v = vec![1, 2, 3];
    assert_eq!(v.len(), 3);
}";

    insta::assert_snapshot!(token_labels(source, &language::rust()));
}
