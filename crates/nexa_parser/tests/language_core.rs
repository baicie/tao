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
    let source = "function main(): Int { return 1 + 2 * 3; }";
    let parse = parse_source(FileId::new(0), source);
    let syntax = parse.syntax();

    assert!(
        parse.is_ok(),
        "unexpected diagnostics: {:?}",
        parse.diagnostics()
    );
    assert_eq!(syntax.to_string(), source);

    let binary_expressions = binary_expressions(&syntax);
    assert_eq!(binary_expressions.len(), 2);

    let addition = &binary_expressions[0];
    let multiplication = &binary_expressions[1];
    assert_eq!(
        direct_node_kinds(addition),
        vec![SyntaxKind::IntLiteral, SyntaxKind::BinaryExpression]
    );
    assert!(has_direct_token(addition, SyntaxKind::Plus));
    assert_eq!(
        direct_node_kinds(multiplication),
        vec![SyntaxKind::IntLiteral, SyntaxKind::IntLiteral]
    );
    assert!(has_direct_token(multiplication, SyntaxKind::Star));
}

#[test]
fn parse_source_builds_left_associative_binary_expressions() {
    let source = "function main(): Int { return 10 - 3 - 2; }";
    let parse = parse_source(FileId::new(0), source);
    let syntax = parse.syntax();

    assert!(
        parse.is_ok(),
        "unexpected diagnostics: {:?}",
        parse.diagnostics()
    );

    let binary_expressions = binary_expressions(&syntax);
    assert_eq!(binary_expressions.len(), 2);

    let outer_subtraction = &binary_expressions[0];
    let inner_subtraction = &binary_expressions[1];
    assert_eq!(
        direct_node_kinds(outer_subtraction),
        vec![SyntaxKind::BinaryExpression, SyntaxKind::IntLiteral]
    );
    assert!(has_direct_token(outer_subtraction, SyntaxKind::Minus));
    assert_eq!(
        direct_node_kinds(inner_subtraction),
        vec![SyntaxKind::IntLiteral, SyntaxKind::IntLiteral]
    );
    assert!(has_direct_token(inner_subtraction, SyntaxKind::Minus));
}

#[test]
fn parse_source_accepts_booleans_else_unannotated_constants_and_comparisons() {
    let source = r#"function main(flag: Bool, value: Int): Unit {
  const inferred = !flag;
  if (true) { return; } else { return; }
  if (-value < value) { return; }
  if (value <= value) { return; }
  if (value > -value) { return; }
  if (value >= value) { return; }
  print(false);
}"#;
    let parse = parse_source(FileId::new(0), source);
    let syntax = parse.syntax();

    assert!(
        parse.is_ok(),
        "unexpected diagnostics: {:?}",
        parse.diagnostics()
    );
    assert_eq!(syntax.to_string(), source);
    assert_eq!(count_nodes(&syntax, SyntaxKind::ElseClause), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::BoolLiteral), 2);
    assert_eq!(count_nodes(&syntax, SyntaxKind::UnaryExpression), 3);
    assert_eq!(count_nodes(&syntax, SyntaxKind::BinaryExpression), 4);

    let const_declarations: Vec<_> = syntax
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::ConstDeclaration)
        .collect();
    assert_eq!(const_declarations.len(), 1);
    assert!(!has_direct_token(&const_declarations[0], SyntaxKind::Colon));
}

#[test]
fn parse_source_accepts_parenthesized_expressions_and_chained_calls() {
    let source = "function main(): Unit { factory()((1 + 2) * 3 / 4); }";
    let parse = parse_source(FileId::new(0), source);
    let syntax = parse.syntax();

    assert!(
        parse.is_ok(),
        "unexpected diagnostics: {:?}",
        parse.diagnostics()
    );
    assert_eq!(syntax.to_string(), source);
    assert_eq!(count_nodes(&syntax, SyntaxKind::ParenthesizedExpression), 1);

    let call_expressions: Vec<_> = syntax
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::CallExpression)
        .collect();
    assert_eq!(call_expressions.len(), 2);
    assert_eq!(
        direct_node_kinds(&call_expressions[0]),
        vec![SyntaxKind::CallExpression, SyntaxKind::ArgumentList]
    );
}

#[test]
fn parse_source_accepts_comma_separated_parameters() {
    let source = "function combine(first: Int, second: Bool, third: Unit): Unit { return; }";
    let parse = parse_source(FileId::new(0), source);
    let syntax = parse.syntax();

    assert!(
        parse.is_ok(),
        "unexpected diagnostics: {:?}",
        parse.diagnostics()
    );
    assert_eq!(count_nodes(&syntax, SyntaxKind::Parameter), 3);
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
fn parse_source_rejects_a_missing_parameter_comma() {
    assert_rejected_with_diagnostic(
        "function main(left: Int right: Int): Unit { return; }",
        "expected `,` or `)` after parameter",
    );
}

#[test]
fn parse_source_rejects_a_trailing_argument_comma() {
    assert_rejected_with_diagnostic(
        "function main(): Unit { print(1,); }",
        "expected expression",
    );
}

#[test]
fn parse_source_rejects_a_missing_call_closing_parenthesis() {
    assert_rejected_with_diagnostic(
        "function main(): Unit { print((1 + 2); }",
        "expected `)` after arguments",
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

fn binary_expressions(syntax: &nexa_syntax::SyntaxNode) -> Vec<nexa_syntax::SyntaxNode> {
    syntax
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::BinaryExpression)
        .collect()
}

fn direct_node_kinds(node: &nexa_syntax::SyntaxNode) -> Vec<SyntaxKind> {
    node.children().map(|child| child.kind()).collect()
}

fn has_direct_token(node: &nexa_syntax::SyntaxNode, kind: SyntaxKind) -> bool {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == kind)
}

fn assert_rejected_with_diagnostic(source: &str, expected_message: &str) {
    let parse = parse_source(FileId::new(0), source);

    assert!(!parse.is_ok(), "parser unexpectedly accepted: {source}");
    assert_eq!(parse.syntax().to_string(), source);
    assert!(
        parse
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message() == expected_message),
        "expected `{expected_message}` in diagnostics: {:?}",
        parse.diagnostics()
    );
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
