//! Rust/Futao lexer differential contract for toolchain 0.0.7.

use nexa_compiler::{
    FutaoLexerAdapter, LexerAdapter, LexerDifferentialHarness, LexerDifferentialOutcome,
    LexerImplementation, LexerObservable, RustLexerAdapter, LEXER_SNAPSHOT_SCHEMA_VERSION,
};

#[test]
fn rust_snapshot_freezes_token_trivia_and_lexical_diagnostic_shapes(
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = RustLexerAdapter.lex("// 涛\n@");
    let snapshot = snapshot?;

    assert_eq!(snapshot.schema_version(), LEXER_SNAPSHOT_SCHEMA_VERSION);
    assert_eq!(snapshot.tokens().len(), 3);
    assert_eq!(
        snapshot
            .tokens()
            .iter()
            .map(|token| (
                token.kind_id(),
                token.start(),
                token.end(),
                token.is_trivia()
            ))
            .collect::<Vec<_>>(),
        [(3, 0, 6, true), (2, 6, 7, true), (11, 7, 8, false)]
    );

    assert_eq!(snapshot.diagnostics().len(), 1);
    let diagnostic = snapshot
        .diagnostics()
        .first()
        .ok_or_else(|| std::io::Error::other("expected one lexical diagnostic"))?;
    assert_eq!(diagnostic.code(), "E1001");
    assert_eq!(diagnostic.severity().as_str(), "error");
    assert_eq!(diagnostic.label_style().as_str(), "primary");
    assert_eq!((diagnostic.start(), diagnostic.end()), (7, 8));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&snapshot.to_json()?)?["schemaVersion"],
        1
    );
    Ok(())
}

#[test]
fn futao_adapter_executes_real_bootstrap_source_and_matches_reference(
) -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustLexerAdapter;
    let futao = FutaoLexerAdapter::new()?;
    let harness = LexerDifferentialHarness::new(&rust, &futao);
    let cases = [
        ("accepted/empty", ""),
        (
            "accepted/core",
            "function main(): Unit { const answer = 40 + 2; } // eof",
        ),
        (
            "accepted/operators",
            "(){}[],;:=+-*/.!&&||===<=>==>|=> type match case default import export from for of",
        ),
        (
            "accepted/utf8",
            "// 波涛\r\nconst text: String = \"Futao 你好\\n\";",
        ),
        (
            "rejected/lexical",
            "@ & == != !== \"bad\\q\" \"unterminated\nnext",
        ),
        (
            "fuzz/long-boundary",
            "identifier_0123456789 // a comment longer than one scan leaf\nfunction after(): Unit {}",
        ),
    ];

    for (case_id, source) in cases {
        let report = harness.run_case(case_id, source)?;
        assert_eq!(report.case_id(), case_id);
        assert_eq!(report.reference(), LexerImplementation::RustReference);
        assert_eq!(report.candidate(), LexerImplementation::Futao);
        assert_eq!(report.outcome(), LexerDifferentialOutcome::Match);
        assert!(report.is_match());
        assert!(report.passes_gate());
        assert!(report.differences().is_empty());
    }
    Ok(())
}

#[test]
fn futao_adapter_rejects_vertical_tab_like_the_rust_lexer() -> Result<(), Box<dyn std::error::Error>>
{
    let rust = RustLexerAdapter;
    let futao = FutaoLexerAdapter::new()?;
    let harness = LexerDifferentialHarness::new(&rust, &futao);

    let report = harness.run_case("rejected/vertical-tab", "\u{000b}")?;

    assert_eq!(report.outcome(), LexerDifferentialOutcome::Match);
    Ok(())
}

#[test]
fn futao_adapter_handles_the_complete_fuzz_byte_budget() -> Result<(), Box<dyn std::error::Error>> {
    let source = "@".repeat(512);
    let rust = RustLexerAdapter;
    let futao = FutaoLexerAdapter::new()?;
    let harness = LexerDifferentialHarness::new(&rust, &futao);

    let report = harness.run_case("fuzz/max-byte-budget", &source)?;

    assert_eq!(report.outcome(), LexerDifferentialOutcome::Match);
    assert_eq!(report.reference_snapshot().tokens().len(), 512);
    assert_eq!(report.reference_snapshot().diagnostics().len(), 512);
    Ok(())
}

#[test]
fn any_observable_difference_fails_the_gate_without_suppression(
) -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustLexerAdapter;
    let different = DifferentLexer;
    let harness = LexerDifferentialHarness::new(&rust, &different);

    let report = harness.run_case("synthetic/difference", "@")?;

    assert_eq!(report.outcome(), LexerDifferentialOutcome::Differences);
    assert!(!report.is_match());
    assert!(!report.passes_gate());
    assert_eq!(report.differences().len(), 2);
    assert_eq!(
        report.differences()[0].observable(),
        LexerObservable::Tokens
    );
    assert_eq!(
        report.differences()[1].observable(),
        LexerObservable::Diagnostics
    );
    assert!(report.differences().iter().all(|difference| {
        difference.reference_digest().starts_with("sha256:")
            && difference.candidate_digest().starts_with("sha256:")
    }));
    Ok(())
}

struct DifferentLexer;

impl LexerAdapter for DifferentLexer {
    fn implementation(&self) -> LexerImplementation {
        LexerImplementation::Futao
    }

    fn lex(
        &self,
        _source: &str,
    ) -> Result<nexa_compiler::LexerSnapshot, nexa_compiler::LexerAdapterError> {
        RustLexerAdapter.lex("/")
    }
}
