//! Accepted, rejected, and recovery tests for named record syntax.

use nexa_parser::parse_source;
use nexa_span::FileId;
use nexa_syntax::{SyntaxKind, SyntaxNode};

#[test]
fn parse_source_accepts_named_records_literals_and_field_access() {
    let source = r#"type Address = {
  city: String;
};

type User = {
  name: String;
  address: Address;
  tags: String[];
};

function main(): Unit {
  const user: User = {
    name: "Ada",
    address: { city: "London" },
    tags: []
  };
  print(user.address.city);
}"#;
    let syntax = assert_parse_ok(source);

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::RecordDeclaration),
            count_nodes(&syntax, SyntaxKind::RecordFieldDeclaration),
            count_nodes(&syntax, SyntaxKind::RecordExpression),
            count_nodes(&syntax, SyntaxKind::RecordFieldInitializer),
        ),
        (2, 4, 2, 4)
    );
}

#[test]
fn parse_source_nests_array_suffixes_around_a_named_type() -> Result<(), Box<dyn std::error::Error>>
{
    let syntax = assert_parse_ok("function read(users: User[][]): User { return users[0][0]; }");
    let outer = first_node(&syntax, SyntaxKind::ArrayType)?;
    let inner = direct_child(&outer, SyntaxKind::ArrayType)?;
    let named = direct_child(&inner, SyntaxKind::Type)?;

    assert_eq!(direct_significant_token_kinds(&named), [SyntaxKind::Ident]);

    Ok(())
}

#[test]
fn record_declaration_keeps_its_body_and_fields_as_direct_children(
) -> Result<(), Box<dyn std::error::Error>> {
    let syntax = assert_parse_ok("type User = { name: String; age: Int; };");
    let declaration = first_node(&syntax, SyntaxKind::RecordDeclaration)?;
    let body = direct_child(&declaration, SyntaxKind::RecordBody)?;
    let first_field = direct_child(&body, SyntaxKind::RecordFieldDeclaration)?;

    assert_eq!(
        (
            direct_node_kinds(&declaration),
            direct_node_kinds(&body),
            direct_node_kinds(&first_field),
            direct_significant_token_kinds(&first_field),
        ),
        (
            vec![SyntaxKind::RecordBody],
            vec![
                SyntaxKind::RecordFieldDeclaration,
                SyntaxKind::RecordFieldDeclaration,
            ],
            vec![SyntaxKind::Type],
            vec![SyntaxKind::Ident, SyntaxKind::Colon, SyntaxKind::Semicolon],
        )
    );

    Ok(())
}

#[test]
fn record_expression_keeps_field_initializers_as_direct_children(
) -> Result<(), Box<dyn std::error::Error>> {
    let syntax =
        assert_parse_ok("function main(): Unit { const user: User = { name: \"Ada\", age: 42 }; }");
    let record = first_node(&syntax, SyntaxKind::RecordExpression)?;

    assert_eq!(
        direct_node_kinds(&record),
        [
            SyntaxKind::RecordFieldInitializer,
            SyntaxKind::RecordFieldInitializer,
        ]
    );

    Ok(())
}

#[test]
fn parse_source_accepts_empty_contextual_record_literals_and_postfix_access() {
    let syntax = assert_parse_ok(
        "type Empty = {}; function main(): Unit { const empty: Empty = {}; ({ name: \"Ada\" }).name; }",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::RecordExpression),
            count_nodes(&syntax, SyntaxKind::MemberExpression),
        ),
        (2, 1)
    );
}

#[test]
fn parser_leaves_record_shape_validation_to_semantic_analysis() {
    assert_parse_ok(
        "type User = { name: String; }; function main(): Unit { const user: User = { other: 1, other: 2 }; }",
    );
}

#[test]
fn parse_source_rejects_a_trailing_record_literal_comma() {
    assert_parse_rejected(
        "function main(): Unit { const user: User = { name: \"Ada\", }; }",
        "expected record field after `,`",
    );
}

#[test]
fn parse_source_rejects_a_missing_record_literal_comma() {
    let syntax = assert_parse_rejected(
        "function main(): Unit { const user: User = { name: \"Ada\" age: 42 }; }",
        "expected `,` or `}` after record field",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::RecordFieldInitializer), 2);
}

#[test]
fn parse_source_rejects_a_comma_between_record_field_declarations() {
    let syntax = assert_parse_rejected(
        "type User = { name: String, age: Int; };",
        "expected `;` after record field declaration",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::RecordFieldDeclaration), 2);
}

#[test]
fn parse_source_rejects_a_missing_record_field_colon() {
    assert_parse_rejected(
        "function main(): Unit { const user: User = { name \"Ada\" }; }",
        "expected `:` after record field name",
    );
}

#[test]
fn parse_source_recovers_a_field_after_a_missing_declaration_semicolon() {
    let syntax = assert_parse_rejected(
        "type User = { name: String age: Int; }; function main(): Unit {}",
        "expected `;` after record field declaration",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::RecordFieldDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (2, 1)
    );
}

#[test]
fn parse_source_recovers_the_next_top_level_items_after_a_missing_record_brace() {
    let source =
        "type Broken = { value: Int; type Good = { value: Int; }; function main(): Unit {}";
    let syntax = assert_parse_rejected(source, "expected `}` after record fields");

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::RecordDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (2, 1)
    );
}

#[test]
fn parse_source_recovers_the_next_statement_after_a_missing_literal_brace() {
    let source = "function main(): Unit { const bad: User = { name: \"Ada\"; const good = 42; }";
    let syntax = assert_parse_rejected(source, "expected `}` after record fields");

    assert_eq!(count_nodes(&syntax, SyntaxKind::ConstDeclaration), 2);
}

#[test]
fn parse_source_advances_past_consecutive_record_literal_commas() {
    let source = "function main(): Unit { const first: User = {, name: \"Ada\"}; const second: User = {name: \"Ada\",, age: 42}; }";
    let syntax = assert_parse_rejected(source, "expected record field name");

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ConstDeclaration),
            count_nodes(&syntax, SyntaxKind::RecordExpression),
        ),
        (2, 2)
    );
}

#[test]
fn missing_if_parenthesis_does_not_turn_the_branch_block_into_a_record() {
    let syntax = assert_parse_rejected(
        "function main(): Unit { if { print(1); } print(2); }",
        "expected `(` after `if`",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::RecordExpression),
            count_nodes(&syntax, SyntaxKind::CallExpression),
        ),
        (0, 2)
    );
}

#[test]
fn parse_source_accepts_a_record_literal_inside_parenthesized_condition_syntax() {
    let syntax = assert_parse_ok(
        "function main(): Unit { if ({ enabled: true }) { print(1); } while ({ ready: false }) {} }",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::RecordExpression), 2);
}

#[test]
fn a_type_declaration_inside_a_block_recovers_as_a_top_level_item() {
    let source =
        "function broken(): Unit { type User = { name: String; }; function main(): Unit {}";
    let syntax = assert_parse_rejected(source, "expected `}` to close block");

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::RecordDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (1, 2)
    );
}

#[test]
fn parse_source_recovers_after_a_missing_record_name() {
    let syntax = assert_parse_rejected(
        "type = { value: Int; }; function main(): Unit {}",
        "expected record name",
    );

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::RecordDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (1, 1)
    );
}

#[test]
fn parse_source_recovers_after_a_missing_record_declaration_semicolon() {
    let syntax = assert_parse_rejected(
        "type User = { value: Int; } function main(): Unit {}",
        "expected `;` after record declaration",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::FunctionDeclaration), 1);
}

#[test]
fn parse_source_treats_type_as_a_reserved_keyword() {
    assert_parse_rejected("function type(): Unit {}", "expected function name");
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
