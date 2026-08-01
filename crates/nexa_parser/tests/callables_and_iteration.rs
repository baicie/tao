//! Accepted, rejected, ambiguity, and recovery tests for callable and iteration syntax.

use nexa_parser::parse_source;
use nexa_span::{FileId, TextRange};
use nexa_syntax::{SyntaxKind, SyntaxNode};

#[test]
fn parse_source_accepts_function_types_arrows_and_for_of() {
    let source = r#"function apply(transform: (value: Int) => Int, value: Int): Int {
  return transform(value);
}

function main(values: Int[]): Unit {
  const increment: (value: Int) => Int = (value: Int): Int => value + 1;
  const identity: (value: Int) => Int = (value: Int): Int => { return value; };
  for (const value of values) {
    print(increment(identity(value)));
  }
}"#;
    let syntax = assert_parse_ok(source);

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::FunctionType),
            count_nodes(&syntax, SyntaxKind::ArrowExpression),
            count_nodes(&syntax, SyntaxKind::ArrowBody),
            count_nodes(&syntax, SyntaxKind::ForStatement),
            count_nodes(&syntax, SyntaxKind::ForBinding),
        ),
        (3, 2, 2, 1, 1)
    );
}

#[test]
fn parse_source_accepts_arrays_of_parenthesized_function_types() {
    let syntax =
        assert_parse_ok("function main(): Unit { const handlers: ((value: Int) => Int)[] = []; }");

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::FunctionType),
            count_nodes(&syntax, SyntaxKind::ArrayType)
        ),
        (1, 1)
    );
}

#[test]
fn callable_cst_has_stable_direct_child_shapes() -> Result<(), Box<dyn std::error::Error>> {
    let syntax = assert_parse_ok(
        "function main(): Unit { const mapper: (value: Int) => Int = (value: Int): Int => value; }",
    );
    let function_type = first_node(&syntax, SyntaxKind::FunctionType)?;
    let arrow = first_node(&syntax, SyntaxKind::ArrowExpression)?;
    let arrow_body = direct_child(&arrow, SyntaxKind::ArrowBody)?;

    assert_eq!(
        (
            direct_node_kinds(&function_type),
            direct_significant_token_kinds(&function_type),
            direct_node_kinds(&arrow),
            direct_significant_token_kinds(&arrow),
            direct_node_kinds(&arrow_body),
        ),
        (
            vec![SyntaxKind::ParameterList, SyntaxKind::Type],
            vec![SyntaxKind::LParen, SyntaxKind::RParen, SyntaxKind::FatArrow],
            vec![
                SyntaxKind::ParameterList,
                SyntaxKind::Type,
                SyntaxKind::ArrowBody,
            ],
            vec![
                SyntaxKind::LParen,
                SyntaxKind::RParen,
                SyntaxKind::Colon,
                SyntaxKind::FatArrow,
            ],
            vec![SyntaxKind::NameReference],
        )
    );

    Ok(())
}

#[test]
fn for_of_cst_keeps_the_binding_and_contextual_of_token_direct() -> Result<(), std::io::Error> {
    let syntax = assert_parse_ok(
        "function main(values: Int[]): Unit { for (const value of values) { print(value); } }",
    );
    let statement = first_node(&syntax, SyntaxKind::ForStatement)?;
    let binding = direct_child(&statement, SyntaxKind::ForBinding)?;
    let direct_ident_text = statement
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::Ident)
        .map(|token| token.text().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        (
            direct_node_kinds(&statement),
            direct_significant_token_kinds(&binding),
            direct_ident_text,
        ),
        (
            vec![
                SyntaxKind::ForBinding,
                SyntaxKind::NameReference,
                SyntaxKind::Block,
            ],
            vec![SyntaxKind::ConstKw, SyntaxKind::Ident],
            vec!["of".to_owned()],
        )
    );

    Ok(())
}

#[test]
fn parenthesized_expressions_are_not_misclassified_as_arrows() {
    let syntax = assert_parse_ok(
        "function main(value: Int): Unit { (value); ((value + 1)); consume((value)); }",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ParenthesizedExpression),
            count_nodes(&syntax, SyntaxKind::ArrowExpression),
        ),
        (4, 0)
    );
}

#[test]
fn of_remains_an_identifier_outside_a_for_header() {
    let syntax =
        assert_parse_ok("function of(value: Int): Int { const often = value; return often; }");

    assert_eq!(count_nodes(&syntax, SyntaxKind::FunctionDeclaration), 1);
}

#[test]
fn parser_leaves_callable_and_iteration_semantics_to_later_phases() {
    assert_parse_ok(
        "function main(values: Int[]): Unit { const value = (item: Int): Bool => item; for (const item of values) {} }",
    );
}

#[test]
fn missing_function_type_arrow_recovers_the_next_parameter() {
    let syntax = assert_parse_rejected(
        "function use(callback: (value: Int) Int, later: Int): Unit { print(later); }",
        "expected `=>` in function type",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::Parameter), 3);
}

#[test]
fn missing_parameter_name_reports_at_the_colon_and_keeps_the_next_function(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function first(: Int): Unit {} function later(): Unit {}";
    let syntax = assert_parse_rejected_at(source, "expected parameter name", ": Int", 1)?;
    let parse = parse_source(FileId::new(0), source);

    assert_eq!(count_nodes(&syntax, SyntaxKind::FunctionDeclaration), 2);
    assert_eq!(parse.diagnostics().len(), 1, "{:?}", parse.diagnostics());

    Ok(())
}

#[test]
fn missing_arrow_token_recovers_the_later_statement() -> Result<(), Box<dyn std::error::Error>> {
    let source = "function main(): Unit { const mapper = (value: Int): Int value; const later = 1; print(later); }";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `=>` after arrow return type",
        "value; const later",
        "value".len(),
    )?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ArrowExpression),
            count_nodes(&syntax, SyntaxKind::ConstDeclaration),
            count_nodes(&syntax, SyntaxKind::CallExpression),
        ),
        (1, 2, 1)
    );

    Ok(())
}

#[test]
fn missing_arrow_return_colon_still_builds_the_arrow_and_recovers() {
    let syntax = assert_parse_rejected(
        "function main(): Unit { const mapper = (value: Int) => value; const later = 1; }",
        "expected `:` before arrow return type",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ArrowExpression),
            count_nodes(&syntax, SyntaxKind::ConstDeclaration),
        ),
        (1, 2)
    );
}

#[test]
fn missing_of_reports_the_collection_and_recovers_the_later_statement(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function main(values: Int[]): Unit { for (const value values) { print(value); } print(2); }";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `of` after `for` binding",
        "values) {",
        "values".len(),
    )?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ForStatement),
            count_nodes(&syntax, SyntaxKind::CallExpression),
        ),
        (1, 2)
    );

    Ok(())
}

#[test]
fn malformed_for_binding_recovers_the_later_statement() {
    let syntax = assert_parse_rejected(
        "function main(values: Int[]): Unit { for (let value of values) {} print(2); }",
        "expected `const` in `for...of` binding",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ForStatement),
            count_nodes(&syntax, SyntaxKind::CallExpression),
        ),
        (1, 1)
    );
}

#[test]
fn missing_for_header_close_keeps_the_body_and_later_statement() {
    let syntax = assert_parse_rejected(
        "function main(values: Int[]): Unit { for (const value of values { print(value); } print(2); }",
        "expected `)` after `for...of` header",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ForStatement),
            count_nodes(&syntax, SyntaxKind::CallExpression),
        ),
        (1, 2)
    );
}

#[test]
fn missing_for_iterable_reports_at_the_header_close_and_recovers(
) -> Result<(), Box<dyn std::error::Error>> {
    let source =
        "function main(): Unit { for (const value of ) {} const later = 1; print(later); }";
    let syntax = assert_parse_rejected_at(source, "expected expression", ") {}", 1)?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ForStatement),
            count_nodes(&syntax, SyntaxKind::ConstDeclaration),
            count_nodes(&syntax, SyntaxKind::CallExpression)
        ),
        (1, 1, 1)
    );

    Ok(())
}

#[test]
fn missing_arrow_body_reports_at_the_semicolon_and_recovers(
) -> Result<(), Box<dyn std::error::Error>> {
    let source =
        "function main(): Unit { const mapper = (): Int => ; const later = 1; print(later); }";
    let syntax = assert_parse_rejected_at(source, "expected expression", "; const later", 1)?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ArrowExpression),
            count_nodes(&syntax, SyntaxKind::ConstDeclaration),
            count_nodes(&syntax, SyntaxKind::CallExpression)
        ),
        (1, 2, 1)
    );

    Ok(())
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

fn assert_parse_rejected_at(
    source: &str,
    expected_message: &str,
    occurrence: &str,
    width: usize,
) -> Result<SyntaxNode, std::io::Error> {
    let expected_start = source
        .find(occurrence)
        .ok_or_else(|| std::io::Error::other(format!("expected `{occurrence}` in test source")))?;
    let expected_range = TextRange::new(expected_start, expected_start + width);
    let parse = parse_source(FileId::new(0), source);
    let syntax = parse.syntax();
    let actual = parse
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.message() == expected_message)
        .and_then(|diagnostic| {
            diagnostic.labels().first().map(|label| {
                (
                    diagnostic.code().as_str(),
                    label.span().file(),
                    label.span().range(),
                )
            })
        });

    assert!(!parse.is_ok(), "parser unexpectedly accepted: {source}");
    assert_eq!(syntax.to_string(), source);
    assert_eq!(actual, Some(("E1001", FileId::new(0), expected_range)));

    Ok(syntax)
}

fn first_node(syntax: &SyntaxNode, kind: SyntaxKind) -> Result<SyntaxNode, std::io::Error> {
    syntax
        .descendants()
        .find(|node| node.kind() == kind)
        .ok_or_else(|| std::io::Error::other(format!("expected {kind:?} node")))
}

fn direct_child(node: &SyntaxNode, kind: SyntaxKind) -> Result<SyntaxNode, std::io::Error> {
    node.children()
        .find(|child| child.kind() == kind)
        .ok_or_else(|| std::io::Error::other(format!("expected direct {kind:?} child")))
}

fn count_nodes(syntax: &SyntaxNode, kind: SyntaxKind) -> usize {
    syntax
        .descendants()
        .filter(|node| node.kind() == kind)
        .count()
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
