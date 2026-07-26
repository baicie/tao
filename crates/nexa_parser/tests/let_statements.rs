//! Accepted and rejected tests for the MVP let-statement grammar.

use nexa_parser::parse_source;
use nexa_span::FileId;
use nexa_syntax::SyntaxKind;

#[test]
fn parse_source_accepts_a_let_statement() {
    let source = "let answer = 42;";
    let parse = parse_source(FileId::new(7), source);
    let syntax = parse.syntax();

    assert!(
        parse.is_ok(),
        "unexpected diagnostics: {:?}",
        parse.diagnostics()
    );
    assert_eq!(syntax.kind(), SyntaxKind::SourceFile);
    assert_eq!(syntax.to_string(), source);
    assert_eq!(
        syntax
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::LetStatement)
            .count(),
        1
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
    let syntax_text = parse.syntax().to_string();

    assert_eq!((parse.is_ok(), syntax_text.as_str()), (true, source));
}

#[test]
fn parse_source_accepts_underscored_identifiers_with_digits() {
    let parse = parse_source(FileId::new(0), "let _next7 = 1;");

    assert!(
        parse.is_ok(),
        "unexpected diagnostics: {:?}",
        parse.diagnostics()
    );
}

#[test]
fn parse_source_preserves_trivia_in_the_cst() {
    let source = "let // name\n_value // equals\n= // integer\n42 // semicolon\n;";
    let parse = parse_source(FileId::new(0), source);
    let syntax_text = parse.syntax().to_string();

    assert_eq!((parse.is_ok(), syntax_text.as_str()), (true, source));
}

#[test]
fn parse_source_accepts_multiple_let_statements() {
    let parse = parse_source(FileId::new(0), "let a = 1; let b = 2;");
    let statement_count = parse
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::LetStatement)
        .count();

    assert_eq!((parse.is_ok(), statement_count), (true, 2));
}

#[test]
fn parse_source_has_a_stable_debug_tree() {
    let parse = parse_source(FileId::new(0), "let answer = 42;");

    assert_eq!(
        parse.debug_tree(),
        concat!(
            "SourceFile@0..16\n",
            "  LetStatement@0..16\n",
            "    LetKw@0..3 \"let\"\n",
            "    Whitespace@3..4 \" \"\n",
            "    Ident@4..10 \"answer\"\n",
            "    Whitespace@10..11 \" \"\n",
            "    Eq@11..12 \"=\"\n",
            "    Whitespace@12..13 \" \"\n",
            "    Int@13..15 \"42\"\n",
            "    Semicolon@15..16 \";\"",
        )
    );
}

#[test]
fn parse_source_reports_a_missing_binding_name() {
    assert_first_diagnostic("let = 42;", "expected binding name", FileId::new(7), 4, 5);
}

#[test]
fn parse_source_reports_a_missing_equals_sign() {
    assert_first_diagnostic("let answer 42;", "expected `=`", FileId::new(7), 11, 13);
}

#[test]
fn parse_source_reports_a_missing_integer_literal() {
    assert_first_diagnostic(
        "let answer = ;",
        "expected integer literal",
        FileId::new(7),
        13,
        14,
    );
}

#[test]
fn parse_source_reports_a_missing_semicolon_at_eof() {
    assert_first_diagnostic("let answer = 42", "expected `;`", FileId::new(7), 15, 15);
}

#[test]
fn parse_source_rejects_a_non_let_top_level_item() {
    assert_first_diagnostic(
        "answer = 42;",
        "expected `let` statement",
        FileId::new(7),
        0,
        6,
    );
}

#[test]
fn parse_source_recovers_at_the_next_let_statement() {
    let source = "let = 1; let ok = 2;";
    let parse = parse_source(FileId::new(0), source);
    let statement_count = parse
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::LetStatement)
        .count();
    let syntax_text = parse.syntax().to_string();

    assert_eq!(
        (
            parse.diagnostics().len(),
            statement_count,
            syntax_text.as_str()
        ),
        (1, 2, source)
    );
}

#[test]
fn parse_source_recovers_at_a_semicolon() {
    assert_recovery_boundary("let wrong stuff; let ok = 2;");
}

#[test]
fn parse_source_recovers_at_the_next_let_without_a_semicolon() {
    assert_recovery_boundary("let wrong stuff let ok = 2;");
}

#[test]
fn parse_source_orders_mixed_diagnostics_by_source_position() {
    let parse = parse_source(FileId::new(0), "answer = 42; @");
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
        [("expected `let` statement", 0), ("unknown token", 13),]
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
            expected_message,
            expected_file,
            expected_start,
            expected_end,
        ))
    );
}

fn assert_recovery_boundary(source: &str) {
    let parse = parse_source(FileId::new(0), source);
    let syntax = parse.syntax();
    let statement_count = syntax
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::LetStatement)
        .count();
    let error_count = syntax
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Error)
        .count();
    let syntax_text = syntax.to_string();

    assert_eq!(
        (
            parse.diagnostics().len(),
            statement_count,
            error_count,
            syntax_text.as_str(),
        ),
        (1, 2, 1, source)
    );
}
