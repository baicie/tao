//! Accepted, rejected, precedence, and recovery tests for immutable data syntax.

use nexa_parser::parse_source;
use nexa_span::{FileId, TextRange};
use nexa_syntax::{SyntaxKind, SyntaxNode};

#[test]
fn parse_source_accepts_strings_recursive_array_types_and_array_literals() {
    let source = r#"function first(values: String[][]): String {
  const message: String = "line\n\"quoted\"\\tail";
  const matrix: Int[][] = [[1, 2], []];
  return values[0][1];
}"#;
    let syntax = assert_parse_ok(source);

    assert_eq!(count_nodes(&syntax, SyntaxKind::StringLiteral), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::ArrayType), 4);
    assert_eq!(count_nodes(&syntax, SyntaxKind::ArrayExpression), 3);
    assert_eq!(count_nodes(&syntax, SyntaxKind::IndexExpression), 2);
}

#[test]
fn parse_source_nests_each_recursive_array_type_layer() {
    let syntax = assert_parse_ok("function main(values: String[][]): Unit {}");
    let array_types = nodes(&syntax, SyntaxKind::ArrayType);

    assert_eq!(array_types.len(), 2);
    assert_eq!(direct_node_kinds(&array_types[0]), [SyntaxKind::ArrayType]);
    assert_eq!(direct_node_kinds(&array_types[1]), [SyntaxKind::Type]);
}

#[test]
fn parse_source_keeps_array_elements_as_direct_expression_children() {
    let syntax = assert_parse_ok("function main(): Unit { const values = [[1], [2 + 3]]; }");
    let arrays = nodes(&syntax, SyntaxKind::ArrayExpression);

    assert_eq!(arrays.len(), 3);
    assert_eq!(
        direct_node_kinds(&arrays[0]),
        [SyntaxKind::ArrayExpression, SyntaxKind::ArrayExpression]
    );
    assert_eq!(direct_node_kinds(&arrays[1]), [SyntaxKind::IntLiteral]);
    assert_eq!(
        direct_node_kinds(&arrays[2]),
        [SyntaxKind::BinaryExpression]
    );
}

#[test]
fn parse_source_builds_call_index_and_member_postfixes_in_source_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let syntax =
        assert_parse_ok("function main(values: Int[]): Unit { factory()(values)[0].length(1); }");
    let outer_call = nodes(&syntax, SyntaxKind::CallExpression)
        .into_iter()
        .next()
        .ok_or_else(|| std::io::Error::other("expected outer call expression"))?;
    let member = direct_child(&outer_call, SyntaxKind::MemberExpression)?;
    let index = direct_child(&member, SyntaxKind::IndexExpression)?;

    assert_eq!(
        direct_node_kinds(&outer_call),
        [SyntaxKind::MemberExpression, SyntaxKind::ArgumentList]
    );
    assert_eq!(direct_node_kinds(&member), [SyntaxKind::IndexExpression]);
    assert_eq!(
        direct_significant_token_kinds(&member),
        [SyntaxKind::Dot, SyntaxKind::Ident]
    );
    assert_eq!(
        direct_node_kinds(&index),
        [SyntaxKind::CallExpression, SyntaxKind::IntLiteral]
    );

    Ok(())
}

#[test]
fn parse_source_gives_postfix_expressions_higher_precedence_than_binary_operators() {
    let syntax = assert_parse_ok(
        "function read(values: Int[]): Int { return values[0].length + make()[1] * 2; }",
    );
    let binaries = nodes(&syntax, SyntaxKind::BinaryExpression);

    assert_eq!(binaries.len(), 2);
    assert_eq!(
        direct_node_kinds(&binaries[0]),
        [SyntaxKind::MemberExpression, SyntaxKind::BinaryExpression]
    );
    assert_eq!(
        direct_node_kinds(&binaries[1]),
        [SyntaxKind::IndexExpression, SyntaxKind::IntLiteral]
    );
}

#[test]
fn parse_source_rejects_an_invalid_string_escape_with_e1001() {
    let source = r#"function main(): Unit { const value = "bad\q"; }"#;
    let parse = parse_source(FileId::new(9), source);

    assert!(!parse.is_ok(), "parser unexpectedly accepted: {source}");
    assert_eq!(parse.syntax().to_string(), source);
    assert!(
        parse
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code().as_str() == "E1001"),
        "diagnostics: {:?}",
        parse.diagnostics()
    );
}

#[test]
fn invalid_string_escape_diagnostic_covers_the_lossless_literal_token(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"function main(): Unit { const value = "bad\q"; }"#;
    let literal_start = source
        .find(r#""bad\q""#)
        .ok_or_else(|| std::io::Error::other("test source should contain its invalid literal"))?;
    let parse = parse_source(FileId::new(9), source);
    let actual_range = parse
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.message() == "unknown token")
        .and_then(|diagnostic| diagnostic.labels().first())
        .map(|label| label.span().range());

    assert_eq!(
        actual_range,
        Some(TextRange::new(
            literal_start,
            literal_start + r#""bad\q""#.len()
        ))
    );

    Ok(())
}

#[test]
fn parse_source_recovers_after_an_unterminated_string() {
    let source = "function main(): Unit {\n  const broken = \"bad\n  const good = [\"ok\"];\n}";
    let syntax = assert_parse_rejected(source, "unknown token");

    assert_eq!(count_nodes(&syntax, SyntaxKind::ConstDeclaration), 2);
    assert_eq!(count_nodes(&syntax, SyntaxKind::ArrayExpression), 1);
}

#[test]
fn parse_source_rejects_a_trailing_array_comma() {
    assert_parse_rejected(
        "function main(): Unit { const values = [1,]; }",
        "expected expression after `,`",
    );
}

#[test]
fn parse_source_rejects_a_missing_array_element_comma() {
    assert_parse_rejected(
        "function main(): Unit { const values = [1 2]; }",
        "expected `,` or `]` after array element",
    );
}

#[test]
fn parse_source_advances_past_consecutive_missing_array_elements() {
    let source = "function main(): Unit { const first = [, 1]; const second = [1,, 2]; }";
    let syntax = assert_parse_rejected(source, "expected expression");

    assert_eq!(count_nodes(&syntax, SyntaxKind::ConstDeclaration), 2);
    assert_eq!(count_nodes(&syntax, SyntaxKind::ArrayExpression), 2);
    assert_eq!(count_nodes(&syntax, SyntaxKind::IntLiteral), 3);
}

#[test]
fn parse_source_recovers_the_next_declaration_after_a_missing_array_bracket() {
    let source = "function main(): Unit { const bad = [1, 2; const good = [3]; }";
    let syntax = assert_parse_rejected(source, "expected `]` after array elements");

    assert_eq!(count_nodes(&syntax, SyntaxKind::ConstDeclaration), 2);
    assert_eq!(count_nodes(&syntax, SyntaxKind::ArrayExpression), 2);
}

#[test]
fn parse_source_recovers_the_next_statement_after_a_missing_index_bracket() {
    let source = "function main(values: Int[]): Unit { values[0; print(1); }";
    let syntax = assert_parse_rejected(source, "expected `]` after index expression");

    assert_eq!(count_nodes(&syntax, SyntaxKind::ExpressionStatement), 2);
    assert_eq!(count_nodes(&syntax, SyntaxKind::IndexExpression), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::CallExpression), 1);
}

#[test]
fn parse_source_recovers_the_next_statement_after_an_empty_index() {
    let source = "function main(values: Int[]): Unit { values[]; print(1); }";
    let syntax = assert_parse_rejected(source, "expected expression");

    assert_eq!(count_nodes(&syntax, SyntaxKind::ExpressionStatement), 2);
    assert_eq!(count_nodes(&syntax, SyntaxKind::IndexExpression), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::CallExpression), 1);
}

#[test]
fn parse_source_recovers_the_next_statement_after_a_missing_member_name() {
    let source = "function main(value: Int): Unit { value.; print(1); }";
    let syntax = assert_parse_rejected(source, "expected member name after `.`");

    assert_eq!(count_nodes(&syntax, SyntaxKind::ExpressionStatement), 2);
    assert_eq!(count_nodes(&syntax, SyntaxKind::MemberExpression), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::CallExpression), 1);
}

#[test]
fn parse_source_recovers_after_a_malformed_call_index_member_chain() {
    let source = "function main(value: Int): Unit { value(1[0].; print(2); }";
    let syntax = assert_parse_rejected(source, "expected `)` after arguments");

    assert_eq!(count_nodes(&syntax, SyntaxKind::ExpressionStatement), 2);
    assert_eq!(count_nodes(&syntax, SyntaxKind::CallExpression), 2);
    assert_eq!(count_nodes(&syntax, SyntaxKind::IndexExpression), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::MemberExpression), 1);
}

#[test]
fn parse_source_reports_a_missing_recursive_array_type_bracket() {
    let syntax = assert_parse_rejected(
        "function main(values: String[): Unit {}",
        "expected `]` after array type",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::ArrayType), 1);
}

fn assert_parse_ok(source: &str) -> SyntaxNode {
    let parse = parse_source(FileId::new(0), source);
    let syntax = parse.syntax();

    assert!(
        parse.is_ok(),
        "unexpected diagnostics for `{source}`: {:?}",
        parse.diagnostics()
    );
    assert_eq!(syntax.to_string(), source);
    syntax
}

fn assert_parse_rejected(source: &str, expected_message: &str) -> SyntaxNode {
    let parse = parse_source(FileId::new(0), source);
    let syntax = parse.syntax();

    assert!(!parse.is_ok(), "parser unexpectedly accepted: {source}");
    assert_eq!(syntax.to_string(), source);
    assert!(
        parse
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message() == expected_message),
        "expected `{expected_message}` in diagnostics: {:?}",
        parse.diagnostics()
    );
    syntax
}

fn nodes(syntax: &SyntaxNode, kind: SyntaxKind) -> Vec<SyntaxNode> {
    syntax
        .descendants()
        .filter(|node| node.kind() == kind)
        .collect()
}

fn count_nodes(syntax: &SyntaxNode, kind: SyntaxKind) -> usize {
    nodes(syntax, kind).len()
}

fn direct_child(node: &SyntaxNode, kind: SyntaxKind) -> Result<SyntaxNode, std::io::Error> {
    node.children()
        .find(|child| child.kind() == kind)
        .ok_or_else(|| std::io::Error::other(format!("expected direct {kind:?} child")))
}

fn direct_node_kinds(node: &SyntaxNode) -> Vec<SyntaxKind> {
    node.children().map(|child| child.kind()).collect()
}

fn direct_significant_token_kinds(node: &SyntaxNode) -> Vec<SyntaxKind> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .map(|token| token.kind())
        .filter(|kind| !kind.is_trivia())
        .collect()
}
