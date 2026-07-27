//! Accepted and rejected tests for the Language Core v0.2 grammar.

use nexa_parser::parse_source;
use nexa_span::FileId;
use nexa_syntax::{SyntaxKind, SyntaxNode};

#[test]
fn parse_source_accepts_a_stateful_control_flow_program() {
    let source = r#"function main(): Unit {
  let count: Int = 0;
  while (count < 10 && true) {
    count = count + 1;
    if (count === 5) {
      continue;
    }
    if (count === 9) {
      break;
    }
  }
}"#;
    let syntax = assert_parse_ok(source);

    assert_eq!(count_nodes(&syntax, SyntaxKind::LetDeclaration), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::AssignmentStatement), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::WhileStatement), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::BreakStatement), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::ContinueStatement), 1);
}

#[test]
fn parse_source_accepts_an_unannotated_let_declaration() {
    let syntax = assert_parse_ok("function main(): Unit { let ready = true; }");
    let declarations = nodes(&syntax, SyntaxKind::LetDeclaration);

    assert_eq!(declarations.len(), 1);
    assert_eq!(
        direct_node_kinds(&declarations[0]),
        vec![SyntaxKind::BoolLiteral]
    );
}

#[test]
fn parse_source_distinguishes_assignments_from_expression_statements() {
    let source = r#"function main(): Unit {
  let value = 0;
  value = value + 1;
  value === 1;
  value;
  observe(value);
}"#;
    let syntax = assert_parse_ok(source);
    let assignments = nodes(&syntax, SyntaxKind::AssignmentStatement);

    assert_eq!(assignments.len(), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::ExpressionStatement), 3);
    assert_eq!(
        direct_significant_token_kinds(&assignments[0]),
        vec![SyntaxKind::Ident, SyntaxKind::Eq, SyntaxKind::Semicolon]
    );
    assert_eq!(
        direct_node_kinds(&assignments[0]),
        vec![SyntaxKind::BinaryExpression]
    );
}

#[test]
fn parse_source_builds_logical_operator_precedence_in_the_cst() {
    let source = concat!(
        "function choose(left: Int, right: Int): Bool { ",
        "return true || false && left === right < 1 + 2 * 3; ",
        "}",
    );
    let syntax = assert_parse_ok(source);
    let expressions = nodes(&syntax, SyntaxKind::BinaryExpression);
    let operators: Vec<_> = expressions.iter().map(direct_binary_operator).collect();

    assert_eq!(expressions.len(), 6);
    assert_eq!(
        operators,
        [
            Some(SyntaxKind::PipePipe),
            Some(SyntaxKind::AmpAmp),
            Some(SyntaxKind::EqEqEq),
            Some(SyntaxKind::Lt),
            Some(SyntaxKind::Plus),
            Some(SyntaxKind::Star),
        ]
    );
    assert_eq!(
        direct_node_kinds(&expressions[0]),
        vec![SyntaxKind::BoolLiteral, SyntaxKind::BinaryExpression]
    );
    assert_eq!(
        direct_node_kinds(&expressions[1]),
        vec![SyntaxKind::BoolLiteral, SyntaxKind::BinaryExpression]
    );
}

#[test]
fn parse_source_does_not_validate_loop_control_context() {
    let syntax = assert_parse_ok("function main(): Unit { break; continue; }");

    assert_eq!(count_nodes(&syntax, SyntaxKind::BreakStatement), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::ContinueStatement), 1);
}

#[test]
fn parse_source_treats_while_as_a_reserved_keyword() {
    assert_parse_rejected("function while(): Unit {}", "expected function name");
}

#[test]
fn parse_source_treats_break_as_a_reserved_keyword() {
    assert_parse_rejected("function break(): Unit {}", "expected function name");
}

#[test]
fn parse_source_treats_continue_as_a_reserved_keyword() {
    assert_parse_rejected("function continue(): Unit {}", "expected function name");
}

#[test]
fn parse_source_rejects_a_let_declaration_without_an_initializer() {
    assert_parse_rejected(
        "function main(): Unit { let value: Int; }",
        "expected `=` after binding name",
    );
}

#[test]
fn parse_source_rejects_an_assignment_without_a_value() {
    assert_parse_rejected(
        "function main(): Unit { let value = 0; value =; }",
        "expected expression",
    );
}

#[test]
fn parse_source_rejects_a_non_identifier_assignment_target() {
    let syntax = assert_parse_rejected(
        "function main(value: Int): Unit { (value) = 1; }",
        "expected `;` after expression",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::AssignmentStatement), 0);
}

#[test]
fn parse_source_rejects_assignment_inside_an_expression() {
    let syntax = assert_parse_rejected(
        "function main(value: Int): Unit { print(value = 1); }",
        "expected `)` after arguments",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::AssignmentStatement), 0);
}

#[test]
fn parse_source_rejects_while_without_parentheses() {
    assert_parse_rejected(
        "function main(): Unit { while true { break; } }",
        "expected `(` after `while`",
    );
}

#[test]
fn parse_source_recovers_continue_after_break_without_a_semicolon() {
    let syntax = assert_parse_rejected(
        "function main(): Unit { while (true) { break continue; } }",
        "expected `;` after `break`",
    );

    assert_eq!(count_nodes(&syntax, SyntaxKind::BreakStatement), 1);
    assert_eq!(count_nodes(&syntax, SyntaxKind::ContinueStatement), 1);
}

#[test]
fn parse_source_rejects_continue_with_a_value() {
    assert_parse_rejected(
        "function main(): Unit { continue 1; }",
        "expected `;` after `continue`",
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

fn nodes(syntax: &SyntaxNode, kind: SyntaxKind) -> Vec<SyntaxNode> {
    syntax
        .descendants()
        .filter(|node| node.kind() == kind)
        .collect()
}

fn count_nodes(syntax: &SyntaxNode, kind: SyntaxKind) -> usize {
    nodes(syntax, kind).len()
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

fn direct_binary_operator(node: &SyntaxNode) -> Option<SyntaxKind> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .map(|token| token.kind())
        .find(|kind| {
            matches!(
                kind,
                SyntaxKind::PipePipe
                    | SyntaxKind::AmpAmp
                    | SyntaxKind::EqEqEq
                    | SyntaxKind::Lt
                    | SyntaxKind::LtEq
                    | SyntaxKind::Gt
                    | SyntaxKind::GtEq
                    | SyntaxKind::Plus
                    | SyntaxKind::Minus
                    | SyntaxKind::Star
                    | SyntaxKind::Slash
            )
        })
}
