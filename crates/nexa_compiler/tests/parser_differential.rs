//! Rust/Futao parser differential contract for toolchain 0.0.8.

use nexa_compiler::{
    FutaoParserAdapter, ParserAdapter, ParserCstEvent, ParserDifferentialHarness,
    ParserDifferentialOutcome, ParserImplementation, ParserObservable, RustParserAdapter,
    PARSER_SNAPSHOT_SCHEMA_VERSION,
};

#[test]
fn rust_snapshot_freezes_balanced_lossless_cst_events() -> Result<(), Box<dyn std::error::Error>> {
    let source = "function main(): Unit {}";
    let snapshot = RustParserAdapter.parse(source)?;

    assert_eq!(snapshot.schema_version(), PARSER_SNAPSHOT_SCHEMA_VERSION);
    assert!(matches!(
        snapshot.cst_events().first(),
        Some(ParserCstEvent::StartNode {
            kind_id: 39,
            offset: 0
        })
    ));
    assert!(matches!(
        snapshot.cst_events().last(),
        Some(ParserCstEvent::FinishNode { offset: 24 })
    ));
    assert!(snapshot.cst_events().iter().any(|event| matches!(
        event,
        ParserCstEvent::StartNode {
            kind_id: 40,
            offset: 0
        }
    )));
    assert!(snapshot.recovery_events().is_empty());
    assert!(snapshot.diagnostics().is_empty());
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&snapshot.to_json()?)?["schemaVersion"],
        1
    );
    Ok(())
}

#[test]
fn rust_snapshot_separates_recovery_regions_from_parser_diagnostics(
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = RustParserAdapter.parse("function main(): Unit { @ }")?;

    assert_eq!(snapshot.recovery_events().len(), 1);
    assert_eq!(
        (
            snapshot.recovery_events()[0].start(),
            snapshot.recovery_events()[0].end()
        ),
        (24, 26)
    );
    assert_eq!(snapshot.diagnostics().len(), 1);
    assert_eq!(snapshot.diagnostics()[0].code(), "E1001");
    assert_eq!(
        (
            snapshot.diagnostics()[0].start(),
            snapshot.diagnostics()[0].end()
        ),
        (24, 25)
    );
    Ok(())
}

#[test]
fn rust_snapshot_preserves_generic_close_token_splitting() -> Result<(), Box<dyn std::error::Error>>
{
    let snapshot = RustParserAdapter.parse("type Box<T>= { value: T; };")?;
    let significant_tokens = snapshot
        .cst_events()
        .iter()
        .filter_map(|event| match event {
            ParserCstEvent::Token {
                kind_id,
                start,
                end,
            } if !matches!(kind_id, 2 | 3) => Some((*kind_id, *start, *end)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(significant_tokens.windows(2).any(|pair| {
        pair[0].0 == 37
            && pair[1].0 == 10
            && pair[0].2 == pair[1].1
            && pair[0].1 == 10
            && pair[1].2 == 12
    }));
    Ok(())
}

#[test]
fn futao_adapter_executes_the_real_parser_and_matches_empty_source(
) -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustParserAdapter;
    let futao = FutaoParserAdapter::new()?;
    let harness = ParserDifferentialHarness::new(&rust, &futao);

    let report = harness.run_case("accepted/empty", "")?;

    assert_eq!(report.reference(), ParserImplementation::RustReference);
    assert_eq!(report.candidate(), ParserImplementation::Futao);
    assert_eq!(report.outcome(), ParserDifferentialOutcome::Match);
    assert!(report.passes_gate());
    Ok(())
}

#[test]
fn futao_adapter_matches_a_function_declaration() -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustParserAdapter;
    let futao = FutaoParserAdapter::new()?;
    let harness = ParserDifferentialHarness::new(&rust, &futao);

    let report = harness.run_case(
        "accepted/function",
        "function main(value: Int): Unit { const answer = value + 2; }",
    )?;

    assert_eq!(report.outcome(), ParserDifferentialOutcome::Match);
    Ok(())
}

#[test]
fn futao_adapter_matches_rejected_statement_recovery() -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustParserAdapter;
    let futao = FutaoParserAdapter::new()?;
    let harness = ParserDifferentialHarness::new(&rust, &futao);

    let report = harness.run_case("rejected/statement", "function main(): Unit { @ }")?;

    assert_eq!(report.outcome(), ParserDifferentialOutcome::Match);
    assert_eq!(report.candidate_snapshot().recovery_events().len(), 1);
    assert_eq!(report.candidate_snapshot().diagnostics().len(), 1);
    Ok(())
}

#[test]
fn futao_adapter_matches_generic_close_token_splitting() -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustParserAdapter;
    let futao = FutaoParserAdapter::new()?;
    let harness = ParserDifferentialHarness::new(&rust, &futao);

    let report = harness.run_case("accepted/generic-split", "type Box<T>= { value: T; };")?;

    assert_eq!(report.outcome(), ParserDifferentialOutcome::Match);
    assert!(report
        .candidate_snapshot()
        .cst_events()
        .windows(2)
        .any(|pair| matches!(
            pair,
            [
                ParserCstEvent::Token {
                    kind_id: 37,
                    start: 10,
                    end: 11
                },
                ParserCstEvent::FinishNode { offset: 11 }
            ]
        )));
    Ok(())
}

#[test]
fn futao_adapter_matches_the_accepted_grammar_surface() -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustParserAdapter;
    let futao = FutaoParserAdapter::new()?;
    let harness = ParserDifferentialHarness::new(&rust, &futao);
    let cases = [
        (
            "accepted/modules-generics",
            "import { core, values } from \"./core.ft\";\nexport function identity<T>(value: T): T { return value; }",
        ),
        (
            "accepted/records-unions",
            "type Pair<T> = { left: T; right: T; };\ntype Choice<T> = | Some(value: T) | None;",
        ),
        (
            "accepted/control-flow",
            "function control(values: Int[]): Int { let total: Int = 0; for (const value of values) { total = total + value; } if (total < 1) {} else {} while (false) { break; } return total; }",
        ),
        (
            "accepted/expressions",
            "function expressions(): Int { const callback = (value: Int): Int => value + 1; const records = [{ value: callback(1) }]; return records[0].value; }",
        ),
        (
            "accepted/match",
            "type Choice = Yes(value: Int) | No;\nfunction unwrap(choice: Choice): Int { return match (choice) { case Choice.Yes(value) => value; case Choice.No() => 0; }; }",
        ),
        (
            "accepted/utf8-trivia",
            "// 波涛\r\nfunction text(): String { const value = \"Futao 你好\"; return value; }",
        ),
    ];

    for (case_id, source) in cases {
        let report = harness.run_case(case_id, source)?;
        assert_eq!(
            report.reference_snapshot(),
            report.candidate_snapshot(),
            "parser snapshots differ for {case_id}"
        );
        assert_eq!(report.outcome(), ParserDifferentialOutcome::Match);
    }
    Ok(())
}

#[test]
fn futao_adapter_matches_the_rejected_recovery_surface() -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustParserAdapter;
    let futao = FutaoParserAdapter::new()?;
    let harness = ParserDifferentialHarness::new(&rust, &futao);
    let cases = [
        (
            "rejected/top-level-continuation",
            "const nope = 1; function after(): Unit {}",
        ),
        (
            "rejected/lists",
            "function broken(value Int): Unit { const values = [1, , 2]; return; }",
        ),
        (
            "rejected/record-continuation",
            "type Broken = { first Int; @ second: Int; }; function after(): Unit {}",
        ),
        (
            "rejected/union-continuation",
            "type Choice = | Good(value Int) | | Bad; function after(): Unit {}",
        ),
        (
            "rejected/match-arm-continuation",
            "function broken(value: Int): Int { return match (value) { case Choice.Bad(,) value; default => 0; }; }",
        ),
    ];

    for (case_id, source) in cases {
        let report = harness.run_case(case_id, source)?;
        assert_eq!(
            report.reference_snapshot(),
            report.candidate_snapshot(),
            "parser snapshots differ for {case_id}"
        );
        assert_eq!(report.outcome(), ParserDifferentialOutcome::Match);
    }
    Ok(())
}

fn assert_futao_parser_matches(
    case_id: &str,
    source: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let rust = RustParserAdapter;
    let futao = FutaoParserAdapter::new()?;
    let harness = ParserDifferentialHarness::new(&rust, &futao);

    let report = harness.run_case(case_id, source)?;
    assert_eq!(
        report.reference_snapshot(),
        report.candidate_snapshot(),
        "parser snapshots differ for {case_id}"
    );
    Ok(())
}

#[test]
fn futao_adapter_preserves_import_recovery_after_repeated_commas(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_futao_parser_matches(
        "rejected/import-repeated-commas",
        "import { , , x } from \"x\";",
    )
}

#[test]
fn futao_adapter_preserves_import_recovery_after_repeated_invalid_tokens(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_futao_parser_matches(
        "rejected/import-repeated-invalid-tokens",
        "import { @, @, x } from \"x\";",
    )
}

#[test]
fn futao_adapter_recovers_vertical_tab_like_the_rust_parser(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_futao_parser_matches("rejected/vertical-tab", "\u{000b}")
}

#[test]
fn futao_adapter_preserves_union_state_across_parser_chunks(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_futao_parser_matches(
        "rejected/union-chunk-without-variant",
        &format!("type Empty = {};", "| ".repeat(17)),
    )
}

#[test]
fn futao_adapter_handles_the_maximum_type_discriminator_budget(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = format!("type T<{}Z>=V;", "A,".repeat(245));
    assert_eq!(source.len(), 502);

    assert_futao_parser_matches("fuzz/max-type-declaration-discriminator", &source)
}

#[test]
fn futao_adapter_matches_nine_nested_parenthesized_expressions(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = format!(
        "function f():Int{{return {}1{};}}",
        "(".repeat(9),
        ")".repeat(9)
    );
    assert!(source.is_ascii());
    assert_eq!(source.len(), 45);
    assert!(source.len() <= 512);

    assert_futao_parser_matches("fuzz/nested-parenthesized-expressions", &source)
}

#[test]
fn futao_adapter_matches_eight_nested_array_expressions() -> Result<(), Box<dyn std::error::Error>>
{
    let source = format!("function f():Unit{{{}1{};}}", "[".repeat(8), "]".repeat(8));
    assert!(source.is_ascii());
    assert_eq!(source.len(), 37);
    assert!(source.len() <= 512);

    assert_futao_parser_matches("fuzz/nested-array-expressions", &source)
}

#[test]
fn futao_adapter_matches_sixty_nested_unary_expressions() -> Result<(), Box<dyn std::error::Error>>
{
    let source = format!("function f():Int{{return {}1;}}", "-".repeat(60));
    assert!(source.is_ascii());
    assert_eq!(source.len(), 87);
    assert!(source.len() <= 512);

    assert_futao_parser_matches("fuzz/nested-unary-expressions", &source)
}

#[test]
fn futao_adapter_matches_the_maximum_legal_pattern_binding_list(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = format!(
        "type U=A();function f(x:I):I{{return match(x){{case U.A({}z)=>0;}};}}",
        "a,".repeat(224)
    );
    assert!(source.is_ascii());
    assert_eq!(source.len(), 511);
    assert!(source.len() <= 512);

    let rust = RustParserAdapter;
    let futao = FutaoParserAdapter::new()?;
    let harness = ParserDifferentialHarness::new(&rust, &futao);
    let report = harness.run_case("fuzz/max-legal-pattern-bindings", &source)?;

    assert!(report.reference_snapshot().diagnostics().is_empty());
    assert_eq!(report.outcome(), ParserDifferentialOutcome::Match);
    Ok(())
}

#[test]
fn futao_adapter_handles_the_complete_parser_byte_budget() -> Result<(), Box<dyn std::error::Error>>
{
    let rust = RustParserAdapter;
    let futao = FutaoParserAdapter::new()?;
    let harness = ParserDifferentialHarness::new(&rust, &futao);
    let cases = [
        ("fuzz/max-recovery", "@".repeat(512)),
        ("fuzz/max-top-level", "type ".repeat(100)),
        (
            "fuzz/max-statements",
            format!("function f(): Unit {{ {} }}", "return;".repeat(60)),
        ),
        (
            "fuzz/max-array",
            format!(
                "function f(): Unit {{ const values = [{}1]; }}",
                "1,".repeat(200)
            ),
        ),
        (
            "fuzz/max-record",
            format!("type Dense = {{ {} }};", "a: Int;".repeat(60)),
        ),
        (
            "fuzz/max-imports",
            format!("import {{ {}z }} from \"x\";", "a,".repeat(150)),
        ),
        (
            "fuzz/max-union-variants",
            format!("type Dense = {}Z;", "A | ".repeat(100)),
        ),
        (
            "fuzz/max-variant-fields",
            format!("type Dense = A({}z: Int);", "a:Int,".repeat(60)),
        ),
        (
            "fuzz/max-parameters",
            format!(
                "function f({}z: Int): Unit {{}}",
                "a:Int,".repeat(60)
            ),
        ),
        (
            "fuzz/max-type-parameters",
            format!("function f<{}Z>(): Unit {{}}", "T,".repeat(100)),
        ),
        (
            "fuzz/max-type-arguments",
            format!("type Dense = {{ value: Box<{}Z>; }};", "T,".repeat(100)),
        ),
        (
            "fuzz/max-type-declaration-discriminator",
            format!("type Dense<{}Z> = Value;", "T,".repeat(100)),
        ),
        (
            "fuzz/max-arguments",
            format!("function f(): Unit {{ call({}1); }}", "1,".repeat(200)),
        ),
        (
            "fuzz/max-record-expression",
            format!(
                "function f(): Unit {{ const value = {{{}z:1}}; }}",
                "a:1,".repeat(100)
            ),
        ),
        (
            "fuzz/max-match-arms",
            format!(
                "function f(x: Int): Int {{ return match (x) {{ {} }}; }}",
                "default => 0;".repeat(30)
            ),
        ),
        (
            "fuzz/max-pattern-bindings",
            format!(
                "function f(x: Int): Int {{ return match (x) {{ case U.A({}z) => 0; default => 1; }}; }}",
                "a,".repeat(100)
            ),
        ),
        (
            "fuzz/nested-record-values",
            format!(
                "function f():Unit{{{}1{};}}",
                "{v:".repeat(120),
                "}".repeat(120)
            ),
        ),
        (
            "fuzz/nested-call-arguments",
            format!(
                "function f():Unit{{{}1{};}}",
                "f(".repeat(160),
                ")".repeat(160)
            ),
        ),
        (
            "fuzz/nested-index-expressions",
            format!(
                "function f():Unit{{{}1{};}}",
                "a[".repeat(160),
                "]".repeat(160)
            ),
        ),
        (
            "fuzz/nested-expression-arrow-bodies",
            format!("function f():Unit{{{}1;}}", "():Int=>".repeat(60)),
        ),
        (
            "fuzz/nested-block-arrow-bodies",
            format!(
                "function f():Unit{{{}1;{}}}",
                "():Int=>{return ".repeat(27),
                "};".repeat(27)
            ),
        ),
        (
            "fuzz/nested-match-values",
            format!(
                "function f(x:Int):Int{{return {}0;{}}}",
                "match(x){default=>".repeat(23),
                "};".repeat(23)
            ),
        ),
        (
            "fuzz/mixed-container-chain",
            format!(
                "function f():Unit{{{}1{};}}",
                "{v:[f(a[".repeat(40),
                "])]}".repeat(40)
            ),
        ),
        (
            "fuzz/max-parenthesized-depth",
            format!(
                "function f():Unit{{{}1{};}}",
                "(".repeat(240),
                ")".repeat(240)
            ),
        ),
        (
            "fuzz/max-array-depth",
            format!(
                "function f():Unit{{{}1{};}}",
                "[".repeat(240),
                "]".repeat(240)
            ),
        ),
        (
            "fuzz/max-unary-depth",
            format!("function f():Int{{return {}1;}}", "-".repeat(480)),
        ),
        (
            "fuzz/nested-function-result-types",
            format!("function f():{}Int{{}}", "()=>".repeat(120)),
        ),
        (
            "fuzz/nested-generic-types",
            format!(
                "type R={{value:{}Int{};}};",
                "Box<".repeat(95),
                ">".repeat(95)
            ),
        ),
        (
            "fuzz/max-array-type-suffixes",
            format!("type R={{value:Int{};}};", "[]".repeat(240)),
        ),
        (
            "fuzz/nested-if-blocks",
            format!(
                "function f():Unit{{{}return;{}}}",
                "if(true){".repeat(48),
                "}".repeat(48)
            ),
        ),
        (
            "fuzz/nested-while-blocks",
            format!(
                "function f():Unit{{{}return;{}}}",
                "while(true){".repeat(37),
                "}".repeat(37)
            ),
        ),
        (
            "fuzz/max-expression-trivia",
            format!("function f():Int{{return 1+{}1;}}", "//\n".repeat(160)),
        ),
        (
            "fuzz/max-expression-recovery",
            format!("function f():Unit{{[{}];}}", "@".repeat(480)),
        ),
        (
            "fuzz/max-arrow-lookahead-trivia",
            format!(
                "function f():Unit{{(x:Int){}:Int=>1;}}",
                "//\n".repeat(159)
            ),
        ),
        (
            "fuzz/max-binary-chain",
            format!("function f(): Int {{ return {}1; }}", "1+".repeat(200)),
        ),
        (
            "fuzz/max-postfix-chain",
            format!("function f(): Unit {{ {}a; }}", "a.".repeat(200)),
        ),
        ("fuzz/max-trivia", "//x\n".repeat(100)),
        (
            "fuzz/max-arrow-parameters",
            format!(
                "function f(): Unit {{ ({}z:Int):Unit => {{}}; }}",
                "a:Int,".repeat(55)
            ),
        ),
    ];

    let mut failures = Vec::new();
    for (case_id, source) in cases {
        assert!(source.len() <= 512, "{case_id} exceeds parser byte budget");
        match harness.run_case(case_id, &source) {
            Ok(report) if report.outcome() == ParserDifferentialOutcome::Match => {}
            Ok(_) => failures.push(format!("parser case `{case_id}` produced differences")),
            Err(error) => failures.push(format!("parser case `{case_id}` failed: {error}")),
        }
    }
    assert!(
        failures.is_empty(),
        "parser differential failures:\n{}",
        failures.join("\n")
    );
    Ok(())
}

#[test]
fn any_parser_observable_difference_fails_without_suppression(
) -> Result<(), Box<dyn std::error::Error>> {
    let reference = SourceParser("function main(): Unit { @ }");
    let different = SourceParser("function main(): Unit { 1 }");
    let harness = ParserDifferentialHarness::new(&reference, &different);

    let report = harness.run_case("synthetic/difference", reference.0)?;

    assert_eq!(report.reference(), ParserImplementation::RustReference);
    assert_eq!(report.candidate(), ParserImplementation::Futao);
    assert_eq!(report.outcome(), ParserDifferentialOutcome::Differences);
    assert!(!report.passes_gate());
    assert_eq!(
        report
            .differences()
            .iter()
            .map(|difference| difference.observable())
            .collect::<Vec<_>>(),
        [
            ParserObservable::Cst,
            ParserObservable::Recovery,
            ParserObservable::Diagnostics
        ]
    );
    assert!(report.differences().iter().all(|difference| {
        difference.reference_digest().starts_with("sha256:")
            && difference.candidate_digest().starts_with("sha256:")
    }));
    Ok(())
}

struct SourceParser(&'static str);

impl ParserAdapter for SourceParser {
    fn implementation(&self) -> ParserImplementation {
        if self.0.contains('@') {
            ParserImplementation::RustReference
        } else {
            ParserImplementation::Futao
        }
    }

    fn parse(
        &self,
        _source: &str,
    ) -> Result<nexa_compiler::ParserSnapshot, nexa_compiler::ParserAdapterError> {
        RustParserAdapter.parse(self.0)
    }
}
