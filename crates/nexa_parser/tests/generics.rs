//! Accepted, rejected, and recovery tests for generic declaration syntax.

use nexa_parser::parse_source;
use nexa_span::{FileId, TextRange};
use nexa_syntax::{SyntaxKind, SyntaxNode};

#[test]
fn parse_source_accepts_generic_functions_records_unions_and_nested_type_arguments() {
    let source = r#"function identity<T>(value: T): T { return value; }
type Box<T> = { value: T; };
type Result<T, E> = Ok(value: T) | Error(error: E);
function consume(value: Result<Box<Int>[], String>): Unit {}"#;
    let syntax = assert_parse_ok(source);

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
            count_nodes(&syntax, SyntaxKind::RecordDeclaration),
            count_nodes(&syntax, SyntaxKind::UnionDeclaration),
            count_nodes(&syntax, SyntaxKind::TypeParameterList),
            count_nodes(&syntax, SyntaxKind::TypeParameter),
            count_nodes(&syntax, SyntaxKind::TypeArgumentList),
        ),
        (2, 1, 1, 3, 4, 2)
    );
}

#[test]
fn generic_cst_keeps_parameters_and_arguments_in_direct_children(
) -> Result<(), Box<dyn std::error::Error>> {
    let syntax = assert_parse_ok(
        "type Result<T, E> = Ok(value: T) | Error(error: E); function use(value: Result<Box<Int>[], String>): Unit {}",
    );
    let declaration = first_node(&syntax, SyntaxKind::UnionDeclaration)?;
    let parameters = direct_child(&declaration, SyntaxKind::TypeParameterList)?;
    let applied = syntax
        .descendants()
        .find(|node| {
            node.kind() == SyntaxKind::Type
                && node
                    .children()
                    .any(|child| child.kind() == SyntaxKind::TypeArgumentList)
        })
        .ok_or_else(|| std::io::Error::other("expected applied named type"))?;
    let arguments = direct_child(&applied, SyntaxKind::TypeArgumentList)?;

    assert_eq!(
        (
            direct_node_kinds(&declaration),
            direct_node_kinds(&parameters),
            direct_significant_token_kinds(&parameters),
            direct_node_kinds(&applied),
            direct_node_kinds(&arguments),
        ),
        (
            vec![
                SyntaxKind::TypeParameterList,
                SyntaxKind::UnionVariant,
                SyntaxKind::UnionVariant,
            ],
            vec![SyntaxKind::TypeParameter, SyntaxKind::TypeParameter],
            vec![SyntaxKind::Lt, SyntaxKind::Comma, SyntaxKind::Gt],
            vec![SyntaxKind::TypeArgumentList],
            vec![SyntaxKind::ArrayType, SyntaxKind::Type],
        )
    );

    Ok(())
}

#[test]
fn parse_source_preserves_trivia_inside_generic_lists() {
    assert_parse_ok(
        "export function choose< // first\n T, // second\n E >(value: Result< T, E >): Result<T, E> { return value; }",
    );
}

#[test]
fn type_parameter_closes_split_from_an_adjacent_declaration_equals() {
    let source = "type Box<T>= { value: T; }; type Option<T>=| Some(value: T) | None();";
    let syntax = assert_parse_ok(source);

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::RecordDeclaration),
            count_nodes(&syntax, SyntaxKind::UnionDeclaration),
        ),
        (1, 1)
    );
}

#[test]
fn type_argument_closes_split_from_an_adjacent_initializer_equals() {
    let source = r#"type Box<T> = { value: T; };
function main(): Unit { const boxed: Box<Int>= { value: 42 }; }"#;
    let syntax = assert_parse_ok(source);

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::TypeArgumentList),
            count_nodes(&syntax, SyntaxKind::ConstDeclaration),
            count_nodes(&syntax, SyntaxKind::RecordExpression),
        ),
        (1, 1, 1)
    );
}

#[test]
fn parser_leaves_duplicate_parameters_and_type_arity_to_semantic_analysis() {
    assert_parse_ok(
        "type Box<T> = { value: T; }; function use<T, T>(value: Box<Int, String>): Unit {}",
    );
}

#[test]
fn angle_brackets_in_expressions_remain_comparison_operators(
) -> Result<(), Box<dyn std::error::Error>> {
    let syntax = assert_parse_ok(
        "function compare(left: Int, middle: Int, right: Int): Unit { left < middle > (right); }",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::BinaryExpression),
            count_nodes(&syntax, SyntaxKind::CallExpression),
            count_nodes(&syntax, SyntaxKind::TypeArgumentList),
        ),
        (2, 0, 0)
    );

    Ok(())
}

#[test]
fn parse_source_rejects_empty_type_parameter_lists_with_a_stable_span(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function identity<>(value: Int): Int { return value; }";
    let _ = assert_parse_rejected_at(source, "expected type parameter", ">(value", ">".len())?;

    Ok(())
}

#[test]
fn parse_source_rejects_trailing_type_parameter_commas_with_a_stable_span(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "type Box<T,> = { value: T; };";
    let _ = assert_parse_rejected_at(
        source,
        "expected type parameter after `,`",
        "> =",
        ">".len(),
    )?;

    Ok(())
}

#[test]
fn parse_source_keeps_parameters_after_a_missing_type_parameter_comma() {
    let syntax = assert_parse_rejected(
        "function choose<T E>(value: T): E { return value; }",
        "expected `,` or `>` after type parameter",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::TypeParameter), 2);
}

#[test]
fn parse_source_recovers_function_and_following_item_after_a_missing_parameter_close(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function identity<T(value: T): T { return value; } function main(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `>` after type parameters",
        "(value",
        "(".len(),
    )?;

    assert_eq!(count_nodes(&syntax, SyntaxKind::FunctionDeclaration), 2);

    Ok(())
}

#[test]
fn parse_source_classifies_generic_types_and_recovers_missing_parameter_closes() {
    let source = "type Box<T = { value: T; }; type Option<T = None() | Some(value: T); function main(): Unit {}";
    let syntax = assert_parse_rejected(source, "expected `>` after type parameters");

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::RecordDeclaration),
            count_nodes(&syntax, SyntaxKind::UnionDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (1, 1, 1)
    );
}

#[test]
fn parse_source_rejects_empty_type_argument_lists_with_a_stable_span(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function consume(value: Box<>): Unit {}";
    let _ = assert_parse_rejected_at(source, "expected type argument", ">):", ">".len())?;

    Ok(())
}

#[test]
fn parse_source_rejects_trailing_type_argument_commas_with_a_stable_span(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function consume(value: Result<Int,>): Unit {}";
    let _ = assert_parse_rejected_at(source, "expected type argument after `,`", ">):", ">".len())?;

    Ok(())
}

#[test]
fn expression_angle_brackets_are_not_explicit_generic_call_syntax(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function main(): Unit { identity<Int>(1); } function later(): Unit {}";
    let syntax = assert_parse_rejected_at(source, "expected expression", "Int", "Int".len())?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
            count_nodes(&syntax, SyntaxKind::CallExpression),
            count_nodes(&syntax, SyntaxKind::TypeArgumentList),
        ),
        (2, 0, 0)
    );

    Ok(())
}

#[test]
fn parse_source_keeps_arguments_after_a_missing_type_argument_comma(
) -> Result<(), Box<dyn std::error::Error>> {
    let syntax = assert_parse_rejected(
        "function consume(value: Result<Int String>): Unit {}",
        "expected `,` or `>` after type argument",
    );
    let arguments = first_node(&syntax, SyntaxKind::TypeArgumentList)?;

    assert_eq!(
        direct_node_kinds(&arguments),
        [SyntaxKind::Type, SyntaxKind::Type]
    );

    Ok(())
}

#[test]
fn parse_source_recovers_fields_after_a_missing_type_argument_close(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "type Broken = { value: Pair<Int, String; next: Int; }; function main(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `>` after type arguments",
        "; next",
        ";".len(),
    )?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::RecordFieldDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (2, 1)
    );

    Ok(())
}

#[test]
fn parse_source_recovers_variants_after_a_missing_type_argument_close() {
    let syntax = assert_parse_rejected(
        "type Option<T> = Some(value: Box<T) | None(); function main(): Unit {}",
        "expected `>` after type arguments",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::UnionVariant),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (2, 1)
    );
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

fn count_nodes(syntax: &SyntaxNode, kind: SyntaxKind) -> usize {
    syntax
        .descendants()
        .filter(|node| node.kind() == kind)
        .count()
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
