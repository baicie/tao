//! Accepted and rejected tests for the Language Core v0.1 grammar.

use nexa_parser::parse_source;
use nexa_span::FileId;
use nexa_syntax::SyntaxKind;

const VALID_PROGRAM: &str = r#"function add(left: Int, right: Int): Int {
  return left + right;
}

function main(): Unit {
  const answer: Int = add(40, 2);
  if (answer === 42) {
    print(answer);
  }
}
"#;

#[test]
fn parse_source_accepts_a_language_core_program() {
    let parse = parse_source(FileId::new(7), VALID_PROGRAM);
    let syntax = parse.syntax();

    assert!(
        parse.is_ok(),
        "unexpected diagnostics: {:?}",
        parse.diagnostics()
    );
    assert_eq!(syntax.kind(), SyntaxKind::SourceFile);
    assert_eq!(syntax.to_string(), VALID_PROGRAM);
    assert_eq!(count_nodes(&syntax, SyntaxKind::FunctionDeclaration), 2);
    assert_eq!(count_nodes(&syntax, SyntaxKind::ConstDeclaration), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::IfStatement), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::CallExpression), 2);
}

#[test]
fn parse_source_accepts_let_as_an_identifier() {
    let parse = parse_source(FileId::new(0), "function let(): Unit { return; }");

    assert!(
        parse.is_ok(),
        "parser diagnostics: {:?}",
        parse.diagnostics()
    );
}

#[test]
fn parse_source_accepts_an_empty_file() {
    let parse = parse_source(FileId::new(0), "");

    assert!(
        parse.is_ok(),
        "unexpected diagnostics: {:?}",
        parse.diagnostics()
    );
}

#[test]
fn parse_source_accepts_trivia_only() {
    let source = "// comment\n\n";
    let parse = parse_source(FileId::new(0), source);

    assert_eq!(
        (parse.is_ok(), parse.syntax().to_string().as_str()),
        (true, source)
    );
}

#[test]
fn parse_source_preserves_trivia_inside_a_function() {
    let source = r#"function // name
_main7 // parameters
() // return separator
: Unit // body
{ // declaration
const // name
_answer: Int // initializer
= 42; // close
}"#;
    let parse = parse_source(FileId::new(0), source);

    assert_eq!(
        (parse.is_ok(), parse.syntax().to_string().as_str()),
        (true, source)
    );
}

#[test]
fn parse_source_builds_binary_expressions_by_precedence() {
    let source = "function main(): Unit { print(1 + 2 * 3); }";
    let parse = parse_source(FileId::new(0), source);
    let syntax = parse.syntax();

    assert!(
        parse.is_ok(),
        "unexpected diagnostics: {:?}",
        parse.diagnostics()
    );
    assert_eq!(count_nodes(&syntax, SyntaxKind::BinaryExpression), 2);
    assert_eq!(syntax.to_string(), source);
}

#[test]
fn parse_source_does_not_type_check_conditions() {
    let source = "function main(): Unit { if (42) { return; } }";
    let parse = parse_source(FileId::new(0), source);

    assert!(
        parse.is_ok(),
        "parser diagnostics: {:?}",
        parse.diagnostics()
    );
}

#[test]
fn parse_source_reports_a_missing_function_name() {
    assert_first_diagnostic(
        "function (): Unit { return; }",
        "expected function name",
        FileId::new(7),
        9,
        10,
    );
}

#[test]
fn parse_source_reports_a_missing_parameter_type() {
    assert_first_diagnostic(
        "function main(value: ): Unit { return; }",
        "expected type `Int`, `Bool`, or `Unit`",
        FileId::new(7),
        21,
        22,
    );
}

#[test]
fn parse_source_reports_a_missing_declaration_semicolon_at_eof() {
    assert_first_diagnostic(
        "function main(): Unit { const answer = 42",
        "expected `;` after declaration",
        FileId::new(7),
        41,
        41,
    );
}

#[test]
fn parse_source_rejects_a_trailing_parameter_comma() {
    assert_first_diagnostic(
        "function main(value: Int,): Unit { return; }",
        "expected parameter after `,`",
        FileId::new(7),
        25,
        26,
    );
}

#[test]
fn parse_source_rejects_a_non_function_top_level_item() {
    assert_first_diagnostic(
        "const answer = 42;",
        "expected `function` declaration",
        FileId::new(7),
        0,
        5,
    );
}

#[test]
fn parse_source_recovers_at_the_next_function_declaration() {
    let source = r#"function broken(): Unit { const answer = ; }
function main(): Unit { return; }"#;
    let parse = parse_source(FileId::new(0), source);
    let syntax = parse.syntax();

    assert_eq!(parse.diagnostics().len(), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::FunctionDeclaration), 2);
    assert_eq!(syntax.to_string(), source);
}

#[test]
fn parse_source_recovers_from_a_function_declaration_inside_a_block() {
    let source = "function main(): Unit { function nested(): Unit {} }";
    let parse = parse_source(FileId::new(0), source);

    assert!(!parse.is_ok());
    assert_eq!(parse.syntax().to_string(), source);
    assert_eq!(
        count_nodes(&parse.syntax(), SyntaxKind::FunctionDeclaration),
        2
    );
}

#[test]
fn parse_source_recovers_the_next_function_after_a_missing_block_brace() {
    let source = r#"function broken(): Unit {
  const answer = 42;
function main(): Unit { return; }"#;
    let parse = parse_source(FileId::new(0), source);

    assert!(!parse.is_ok());
    assert_eq!(parse.syntax().to_string(), source);
    assert_eq!(
        count_nodes(&parse.syntax(), SyntaxKind::FunctionDeclaration),
        2
    );
}

#[test]
fn parse_source_orders_lexical_and_parser_diagnostics_by_source_position() {
    let parse = parse_source(
        FileId::new(0),
        "const answer = @; function main(): Unit { return; }",
    );
    let diagnostics: Vec<_> = parse
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| {
            diagnostic
                .labels()
                .first()
                .map(|label| (diagnostic.message(), label.span().range().start()))
        })
        .collect();

    assert_eq!(
        diagnostics,
        [
            ("expected `function` declaration", 0),
            ("unknown token", 15),
        ]
    );
}

fn count_nodes(syntax: &nexa_syntax::SyntaxNode, kind: SyntaxKind) -> usize {
    syntax
        .descendants()
        .filter(|node| node.kind() == kind)
        .count()
}

fn assert_first_diagnostic(
    source: &str,
    expected_message: &str,
    expected_file: FileId,
    expected_start: usize,
    expected_end: usize,
) {
    let parse = parse_source(expected_file, source);
    let actual = parse.diagnostics().first().and_then(|diagnostic| {
        diagnostic.labels().first().map(|label| {
            (
                diagnostic.code().as_str(),
                diagnostic.message(),
                label.span().file(),
                label.span().range().start(),
                label.span().range().end(),
            )
        })
    });

    assert_eq!(
        actual,
        Some((
            "E1001",
            expected_message,
            expected_file,
            expected_start,
            expected_end,
        ))
    );
}
