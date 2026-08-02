//! Parser-only diagnostic boundary for phase differential comparison.

use nexa_parser::parse_source;
use nexa_span::{FileId, TextRange};

#[test]
fn parser_diagnostics_exclude_lexer_owned_unknown_token_reports() {
    let parse = parse_source(FileId::new(0), "@");

    assert!(parse.parser_diagnostics().is_empty());
    assert_eq!(parse.diagnostics().len(), 1);
    assert_eq!(parse.diagnostics()[0].message(), "unknown token");
}

#[test]
fn parser_diagnostics_preserve_parser_ranges_and_combined_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let file = FileId::new(7);
    let parse = parse_source(file, "function main(): Unit { @ }");

    assert_eq!(parse.parser_diagnostics().len(), 1);
    let parser_diagnostic = parse
        .parser_diagnostics()
        .first()
        .ok_or_else(|| std::io::Error::other("expected one parser diagnostic"))?;
    let label = parser_diagnostic
        .labels()
        .first()
        .ok_or_else(|| std::io::Error::other("expected one parser label"))?;
    assert_eq!(parser_diagnostic.message(), "expected expression");
    assert_eq!(label.span().file(), file);
    assert_eq!(label.span().range(), TextRange::new(24, 25));
    assert_eq!(
        parse
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message())
            .collect::<Vec<_>>(),
        ["unknown token", "expected expression"]
    );
    Ok(())
}
