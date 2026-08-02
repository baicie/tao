//! Frozen Rust lexer observables for Futao differential comparison.

use nexa_diagnostics::{LabelStyle, Severity};
use nexa_parser::lex_source;
use nexa_span::{FileId, TextRange};
use nexa_syntax::SyntaxKind;

#[test]
fn lex_source_returns_lossless_tokens_and_only_lexical_diagnostics(
) -> Result<(), Box<dyn std::error::Error>> {
    let file = FileId::new(7);
    let source = "function @(): Unit {\n  // 波涛\n}\n";

    let lexed = lex_source(file, source);

    assert_eq!(
        lexed
            .tokens()
            .iter()
            .map(|token| (token.kind(), token.range(), token.kind().is_trivia()))
            .collect::<Vec<_>>(),
        [
            (SyntaxKind::FunctionKw, TextRange::new(0, 8), false),
            (SyntaxKind::Whitespace, TextRange::new(8, 9), true),
            (SyntaxKind::Unknown, TextRange::new(9, 10), false),
            (SyntaxKind::LParen, TextRange::new(10, 11), false),
            (SyntaxKind::RParen, TextRange::new(11, 12), false),
            (SyntaxKind::Colon, TextRange::new(12, 13), false),
            (SyntaxKind::Whitespace, TextRange::new(13, 14), true),
            (SyntaxKind::UnitKw, TextRange::new(14, 18), false),
            (SyntaxKind::Whitespace, TextRange::new(18, 19), true),
            (SyntaxKind::LBrace, TextRange::new(19, 20), false),
            (SyntaxKind::Whitespace, TextRange::new(20, 23), true),
            (SyntaxKind::LineComment, TextRange::new(23, 32), true),
            (SyntaxKind::Whitespace, TextRange::new(32, 33), true),
            (SyntaxKind::RBrace, TextRange::new(33, 34), false),
            (SyntaxKind::Whitespace, TextRange::new(34, 35), true),
        ]
    );
    assert_eq!(
        lexed
            .tokens()
            .iter()
            .map(|token| token.text())
            .collect::<String>(),
        source
    );

    assert_eq!(lexed.diagnostics().len(), 1);
    let diagnostic = lexed
        .diagnostics()
        .first()
        .ok_or_else(|| std::io::Error::other("expected one lexical diagnostic"))?;
    assert_eq!(diagnostic.labels().len(), 1);
    let label = diagnostic
        .labels()
        .first()
        .ok_or_else(|| std::io::Error::other("expected one lexical label"))?;
    assert_eq!(diagnostic.code().as_str(), "E1001");
    assert_eq!(diagnostic.severity(), Severity::Error);
    assert_eq!(label.style(), LabelStyle::Primary);
    assert_eq!(label.span().file(), file);
    assert_eq!(label.span().range(), TextRange::new(9, 10));
    Ok(())
}

#[test]
fn lex_source_does_not_include_parser_recovery_diagnostics() {
    let lexed = lex_source(FileId::new(0), "function");

    assert!(lexed.diagnostics().is_empty());
    assert_eq!(lexed.tokens().len(), 1);
    assert_eq!(lexed.tokens()[0].kind(), SyntaxKind::FunctionKw);
}
