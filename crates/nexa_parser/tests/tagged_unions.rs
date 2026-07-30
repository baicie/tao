//! Accepted, rejected, and recovery tests for tagged unions and match expressions.

use nexa_parser::parse_source;
use nexa_span::{FileId, TextRange};
use nexa_syntax::{SyntaxKind, SyntaxNode};

#[test]
fn parse_source_accepts_tagged_unions_and_match_expressions() {
    let source = r#"type IntList =
  | Empty()
  | Node(head: Int, tail: IntList);

function sum(list: IntList): Int {
  return match (list) {
    case IntList.Empty() => 0;
    case IntList.Node(head, tail) => head + sum(tail);
  };
}"#;
    let syntax = assert_parse_ok(source);

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::UnionDeclaration),
            count_nodes(&syntax, SyntaxKind::UnionVariant),
            count_nodes(&syntax, SyntaxKind::VariantPayload),
            count_nodes(&syntax, SyntaxKind::MatchExpression),
            count_nodes(&syntax, SyntaxKind::MatchArm),
            count_nodes(&syntax, SyntaxKind::VariantPattern),
            count_nodes(&syntax, SyntaxKind::PatternBindingList),
        ),
        (1, 2, 2, 1, 2, 2, 2)
    );
}

#[test]
fn parse_source_accepts_a_union_without_a_leading_pipe() {
    let syntax = assert_parse_ok("type Result = Ok(value: Int) | Error(message: String);");

    assert_eq!(count_nodes(&syntax, SyntaxKind::UnionVariant), 2);
}

#[test]
fn union_declaration_keeps_variants_and_payloads_in_distinct_cst_nodes(
) -> Result<(), Box<dyn std::error::Error>> {
    let syntax = assert_parse_ok("type IntList = | Empty() | Node(head: Int, tail: IntList);");
    let declaration = first_node(&syntax, SyntaxKind::UnionDeclaration)?;
    let variants = direct_children(&declaration, SyntaxKind::UnionVariant);
    let node_payload = direct_child(&variants[1], SyntaxKind::VariantPayload)?;

    assert_eq!(
        (
            direct_node_kinds(&declaration),
            direct_node_kinds(&variants[0]),
            direct_node_kinds(&node_payload),
            direct_significant_token_kinds(&node_payload),
        ),
        (
            vec![SyntaxKind::UnionVariant, SyntaxKind::UnionVariant],
            vec![SyntaxKind::VariantPayload],
            vec![SyntaxKind::Type, SyntaxKind::Type],
            vec![
                SyntaxKind::LParen,
                SyntaxKind::Ident,
                SyntaxKind::Colon,
                SyntaxKind::Comma,
                SyntaxKind::Ident,
                SyntaxKind::Colon,
                SyntaxKind::RParen,
            ],
        )
    );

    Ok(())
}

#[test]
fn match_expression_keeps_patterns_bindings_and_values_in_direct_children(
) -> Result<(), Box<dyn std::error::Error>> {
    let syntax = assert_parse_ok(
        "function read(value: IntList): Int { return match (value) { case IntList.Node(head, tail) => head; default => 0; }; }",
    );
    let match_expression = first_node(&syntax, SyntaxKind::MatchExpression)?;
    let arms = direct_children(&match_expression, SyntaxKind::MatchArm);
    let pattern = direct_child(&arms[0], SyntaxKind::VariantPattern)?;
    let bindings = direct_child(&pattern, SyntaxKind::PatternBindingList)?;

    assert_eq!(
        (
            direct_node_kinds(&match_expression),
            direct_node_kinds(&arms[0]),
            direct_node_kinds(&arms[1]),
            direct_node_kinds(&pattern),
            direct_significant_token_kinds(&bindings),
        ),
        (
            vec![
                SyntaxKind::NameReference,
                SyntaxKind::MatchArm,
                SyntaxKind::MatchArm,
            ],
            vec![SyntaxKind::VariantPattern, SyntaxKind::NameReference],
            vec![SyntaxKind::IntLiteral],
            vec![SyntaxKind::PatternBindingList],
            vec![
                SyntaxKind::LParen,
                SyntaxKind::Ident,
                SyntaxKind::Comma,
                SyntaxKind::Ident,
                SyntaxKind::RParen,
            ],
        )
    );

    Ok(())
}

#[test]
fn parse_source_preserves_comments_and_whitespace_inside_union_and_match_nodes() {
    let source = "type Choice =\n  | None(// empty\n  )\n  | Some( value : Int );\nfunction read(value: Choice): Int { return match // subject\n( value ) { case Choice.Some( item ) // arrow\n=> item ; default => 0 ; }; }";

    assert_parse_ok(source);
}

#[test]
fn parser_leaves_union_names_arities_duplicates_and_exhaustiveness_to_semantics() {
    assert_parse_ok(
        "type Choice = | Same() | Same(value: Int); function main(): Int { return match (missing) { default => 0; case Other.Unknown(first, second) => 1; default => 2; }; }",
    );
}

#[test]
fn match_is_a_primary_expression_below_binary_operators() -> Result<(), Box<dyn std::error::Error>>
{
    let syntax = assert_parse_ok(
        "function read(value: Choice): Int { return match (value) { default => 1; } + 2; }",
    );
    let binary = first_node(&syntax, SyntaxKind::BinaryExpression)?;

    assert_eq!(
        direct_node_kinds(&binary),
        [SyntaxKind::MatchExpression, SyntaxKind::IntLiteral]
    );

    Ok(())
}

#[test]
fn parse_source_accepts_nested_match_expressions() {
    assert_parse_ok(
        "type Flag = Off() | On(); function read(value: Flag): Int { return match (value) { case Flag.Off() => match (value) { default => 1; }; default => 0; }; }",
    );
}

#[test]
fn parse_source_keeps_items_after_an_empty_match_body() {
    let source = "function read(value: Choice): Unit { const result = match (value) {}; const after = 1; } function later(): Unit {}";
    let syntax = assert_parse_ok(source);

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::MatchExpression),
            count_nodes(&syntax, SyntaxKind::MatchArm),
            count_nodes(&syntax, SyntaxKind::ConstDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (1, 0, 2, 2)
    );
}

#[test]
fn parse_source_reports_a_missing_match_opening_parenthesis_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function read(value: Choice): Unit { const result = match value) { default => 0; }; const after = 1; } function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `(` after `match`",
        "value) {",
        "value".len(),
    )?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::MatchArm),
            count_nodes(&syntax, SyntaxKind::ConstDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (1, 2, 2)
    );

    Ok(())
}

#[test]
fn parse_source_reports_a_missing_match_closing_parenthesis_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function read(value: Choice): Unit { const result = match (value { default => 0; }; const after = 1; } function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `)` after match scrutinee",
        "{ default",
        "{".len(),
    )?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::MatchArm),
            count_nodes(&syntax, SyntaxKind::ConstDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (1, 2, 2)
    );

    Ok(())
}

#[test]
fn parse_source_reports_a_missing_pattern_opening_parenthesis_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function read(value: Choice): Unit { const result = match (value) { case Choice.None => 0; case Choice.Some(item) => item; default => 1; }; const after = 2; } function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `(` after variant pattern",
        "=> 0",
        "=>".len(),
    )?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::MatchArm),
            count_nodes(&syntax, SyntaxKind::ConstDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (3, 2, 2)
    );

    Ok(())
}

#[test]
fn parse_source_reports_a_missing_pattern_closing_parenthesis_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function read(value: Choice): Unit { const result = match (value) { case Choice.Some(item => item; case Choice.None() => 0; default => 1; }; const after = 2; } function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `)` after pattern bindings",
        "=> item",
        "=>".len(),
    )?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::MatchArm),
            count_nodes(&syntax, SyntaxKind::ConstDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (3, 2, 2)
    );

    Ok(())
}

#[test]
fn parse_source_rejects_a_trailing_union_pipe() {
    assert_parse_rejected(
        "type Choice = | None() |;",
        "expected union variant after `|`",
    );
}

#[test]
fn parse_source_rejects_a_trailing_variant_payload_comma() {
    assert_parse_rejected(
        "type Choice = Some(value: Int,);",
        "expected variant field after `,`",
    );
}

#[test]
fn parse_source_rejects_a_trailing_pattern_binding_comma() {
    assert_parse_rejected(
        "function read(value: Choice): Int { return match (value) { case Choice.Some(item,) => item; default => 0; }; }",
        "expected pattern binding after `,`",
    );
}

#[test]
fn parse_source_reports_a_missing_union_name_and_recovers_the_next_declaration() {
    let syntax = assert_parse_rejected(
        "type = | None(); function main(): Unit {}",
        "expected union name",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::FunctionDeclaration), 1);
}

#[test]
fn parse_source_recovers_the_next_variant_after_a_missing_variant_name() {
    let syntax = assert_parse_rejected(
        "type Choice = | () | Some(value: Int); function main(): Unit {}",
        "expected union variant",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::UnionVariant),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (1, 1)
    );
}

#[test]
fn parse_source_reports_a_missing_variant_payload_name() {
    let syntax = assert_parse_rejected(
        "type Choice = Some(: Int) | None();",
        "expected variant field name",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::UnionVariant), 2);
}

#[test]
fn parse_source_reports_a_missing_variant_payload_colon() {
    let syntax = assert_parse_rejected(
        "type Choice = Some(value Int) | None();",
        "expected `:` after variant field name",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::UnionVariant), 2);
}

#[test]
fn parse_source_reports_missing_variant_parentheses_and_recovers_the_next_variant() {
    let syntax = assert_parse_rejected(
        "type Choice = None | Some(); function main(): Unit {}",
        "expected `(` after union variant name",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::UnionVariant),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (2, 1)
    );
}

#[test]
fn parse_source_reports_a_missing_match_arrow_and_keeps_later_arms() {
    let syntax = assert_parse_rejected(
        "function read(value: Choice): Int { return match (value) { case Choice.None() 0; default => 1; }; }",
        "expected `=>` before match arm value",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::MatchArm), 2);
}

#[test]
fn parse_source_recovers_a_variant_after_a_missing_pipe() {
    let syntax = assert_parse_rejected(
        "type Choice = None() Some(value: Int); function main(): Unit {}",
        "expected `|` between union variants",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::UnionVariant),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (2, 1)
    );
}

#[test]
fn parse_source_recovers_an_arm_after_a_missing_semicolon() {
    let syntax = assert_parse_rejected(
        "function read(value: Choice): Int { return match (value) { case Choice.None() => 0 case Choice.Some(item) => item; }; }",
        "expected `;` after match arm",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::MatchArm), 2);
}

#[test]
fn parse_source_recovers_after_consecutive_union_separators() {
    let syntax = assert_parse_rejected(
        "type Choice = | | None() | Some(value: Int); function main(): Unit {}",
        "expected union variant",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::UnionVariant),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (2, 1)
    );
}

#[test]
fn parse_source_recovers_after_consecutive_pattern_commas() {
    let syntax = assert_parse_rejected(
        "function read(value: Choice): Int { return match (value) { case Choice.Some(first,, second) => first; default => 0; }; }",
        "expected pattern binding",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::MatchArm), 2);
}

#[test]
fn parse_source_keeps_pattern_bindings_after_a_missing_comma(
) -> Result<(), Box<dyn std::error::Error>> {
    let syntax = assert_parse_rejected(
        "function read(value: Choice): Int { return match (value) { case Choice.Pair(first second) => first; default => 0; }; }",
        "expected `,` or `)` after pattern binding",
    );
    let bindings = first_node(&syntax, SyntaxKind::PatternBindingList)?;

    assert_eq!(
        direct_significant_token_kinds(&bindings),
        [
            SyntaxKind::LParen,
            SyntaxKind::Ident,
            SyntaxKind::Ident,
            SyntaxKind::RParen,
        ]
    );

    Ok(())
}

#[test]
fn parse_source_recovers_the_next_top_level_item_after_a_missing_union_semicolon() {
    let syntax = assert_parse_rejected(
        "type Choice = None() function main(): Unit {}",
        "expected `;` after union declaration",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::FunctionDeclaration), 1);
}

#[test]
fn parse_source_recovers_the_next_statement_after_a_missing_match_brace() {
    let syntax = assert_parse_rejected(
        "function main(): Unit { const first = match (value) { default => 0; const second = 2; print(second); }",
        "expected `}` after match arms",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ConstDeclaration),
            count_nodes(&syntax, SyntaxKind::CallExpression),
        ),
        (2, 1)
    );
}

#[test]
fn parse_source_recovers_the_next_arm_after_unexpected_match_body_tokens() {
    let syntax = assert_parse_rejected(
        "function read(value: Choice): Int { return match (value) { @@@ case Choice.None() => 0; default => 1; }; }",
        "expected `case`, `default`, or `}` in match",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::MatchArm), 2);
}

#[test]
fn parse_source_treats_match_as_a_reserved_keyword() {
    assert_parse_rejected("function match(): Unit {}", "expected function name");
}

#[test]
fn parse_source_treats_case_as_a_reserved_keyword() {
    assert_parse_rejected("function case(): Unit {}", "expected function name");
}

#[test]
fn parse_source_treats_default_as_a_reserved_keyword() {
    assert_parse_rejected("function default(): Unit {}", "expected function name");
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

fn direct_children(node: &SyntaxNode, kind: SyntaxKind) -> Vec<SyntaxNode> {
    node.children()
        .filter(|child| child.kind() == kind)
        .collect()
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
