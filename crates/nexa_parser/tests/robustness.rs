//! Deterministic, bounded parser robustness regression coverage.

use nexa_parser::{parse_source, Parse};
use nexa_span::FileId;
use nexa_syntax::SyntaxKind;

const TEST_FILE: FileId = FileId::new(90);
const REPLAY_COUNT: usize = 3;
const MAX_SEED_BYTES: usize = 4_096;
const MAX_SEED_DIAGNOSTICS: usize = 256;
const MAX_TRUNCATION_BOUNDARIES: usize = 512;
const MAX_RECOVERY_CASES: usize = 64;
const MAX_DIAGNOSTICS_PER_RECOVERY_CASE: usize = 4;

struct Seed {
    name: &'static str,
    source: &'static str,
}

const SEEDS: &[Seed] = &[
    Seed {
        name: "accepted_v08",
        source: include_str!("corpus/robustness/accepted_v08.nexa"),
    },
    Seed {
        name: "malformed_delimiters",
        source: include_str!("corpus/robustness/malformed_delimiters.nexa"),
    },
    Seed {
        name: "reserved_parameter_names",
        source: include_str!("corpus/robustness/reserved_parameter_names.nexa"),
    },
    Seed {
        name: "utf8_trivia",
        source: include_str!("corpus/robustness/utf8_trivia.nexa"),
    },
    Seed {
        name: "unterminated_utf8_string",
        source: include_str!("corpus/robustness/unterminated_utf8_string.nexa"),
    },
    Seed {
        name: "punctuation_storm",
        source: include_str!("corpus/robustness/punctuation_storm.nexa"),
    },
];

#[test]
fn fixed_seed_corpus_replays_identically() {
    for seed in SEEDS {
        assert!(
            seed.source.len() <= MAX_SEED_BYTES,
            "seed `{}` exceeds its byte budget",
            seed.name
        );

        let expected = parse_source(TEST_FILE, seed.source);
        assert_parse_invariants(seed.name, seed.source, &expected);

        for replay in 1..REPLAY_COUNT {
            let actual = parse_source(TEST_FILE, seed.source);
            assert_eq!(
                actual, expected,
                "seed `{}` changed on replay {replay}",
                seed.name
            );
            assert_eq!(
                actual.debug_tree(),
                expected.debug_tree(),
                "seed `{}` produced a different CST dump on replay {replay}",
                seed.name
            );
        }
    }
}

#[test]
fn utf8_prefix_truncations_are_lossless_span_safe_and_deterministic() {
    let source = include_str!("corpus/robustness/utf8_trivia.nexa");
    let boundaries = source
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(source.len()))
        .collect::<Vec<_>>();

    assert!(
        boundaries.len() <= MAX_TRUNCATION_BOUNDARIES,
        "UTF-8 truncation corpus has {} boundaries, limit is {MAX_TRUNCATION_BOUNDARIES}",
        boundaries.len()
    );

    for boundary in boundaries {
        let prefix = &source[..boundary];
        let expected = parse_source(TEST_FILE, prefix);
        let actual = parse_source(TEST_FILE, prefix);

        assert_parse_invariants("utf8_trivia_prefix", prefix, &expected);
        assert_eq!(
            actual, expected,
            "parse changed at byte boundary {boundary}"
        );
    }
}

#[test]
fn trivia_layouts_preserve_the_same_significant_cst_shape() {
    const SOURCES: &[&str] = &[
        "function main(): Unit { const value = 1; print(value); }",
        "function\tmain ( ) : Unit\r\n{\r\n\tconst value=1 ;\r\nprint ( value ) ;\r\n}",
        "// before\nfunction // name follows\nmain(): Unit { // body\nconst value // initializer\n= 1; print( // argument\nvalue); } // after\n",
        "// \u{4e2d}\u{6587}\u{6ce8}\u{91ca}\u{1f642}\nfunction main(): Unit { const value = 1; print(value); }",
    ];

    let baseline = parse_source(TEST_FILE, SOURCES[0]);
    assert!(
        baseline.is_ok(),
        "diagnostics: {:?}",
        baseline.diagnostics()
    );
    let expected_nodes = significant_node_kinds(&baseline);
    let expected_tokens = significant_tokens(&baseline);

    for (index, source) in SOURCES.iter().enumerate() {
        let parse = parse_source(TEST_FILE, source);

        assert!(
            parse.is_ok(),
            "trivia layout {index} diagnostics: {:?}",
            parse.diagnostics()
        );
        assert_parse_invariants("trivia_layout", source, &parse);
        assert_eq!(significant_node_kinds(&parse), expected_nodes);
        assert_eq!(significant_tokens(&parse), expected_tokens);
    }
}

#[test]
fn bounded_recovery_stress_retains_every_later_declaration() {
    const MALFORMED_FUNCTION: &str =
        "function broken(: Int): Unit { const values = [1,, 2]; print(values); }\n";
    const FINAL_FUNCTION: &str = "function final(): Unit { print(42); }\n";
    let mut source =
        String::with_capacity(MALFORMED_FUNCTION.len() * MAX_RECOVERY_CASES + FINAL_FUNCTION.len());
    for _ in 0..MAX_RECOVERY_CASES {
        source.push_str(MALFORMED_FUNCTION);
    }
    source.push_str(FINAL_FUNCTION);

    let expected = parse_source(TEST_FILE, &source);
    let actual = parse_source(TEST_FILE, &source);
    let function_count = expected
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::FunctionDeclaration)
        .count();

    assert_parse_invariants("bounded_recovery_stress", &source, &expected);
    assert_eq!(actual, expected);
    assert_eq!(function_count, MAX_RECOVERY_CASES + 1);
    assert!(
        expected.diagnostics().len() >= MAX_RECOVERY_CASES,
        "expected at least one diagnostic per malformed function"
    );
    assert!(
        expected.diagnostics().len() <= MAX_RECOVERY_CASES * MAX_DIAGNOSTICS_PER_RECOVERY_CASE,
        "diagnostic amplification exceeded the fixed per-case budget: {}",
        expected.diagnostics().len()
    );
}

fn assert_parse_invariants(name: &str, source: &str, parse: &Parse) {
    assert_eq!(
        parse.syntax().to_string(),
        source,
        "seed `{name}` was not lossless"
    );
    assert!(
        parse.diagnostics().len() <= MAX_SEED_DIAGNOSTICS,
        "seed `{name}` exceeded its diagnostic budget: {}",
        parse.diagnostics().len()
    );

    for token in parse.tokens() {
        let range = token.range();
        assert!(
            range.end() <= source.len(),
            "seed `{name}` token range {range:?} exceeds {} bytes",
            source.len()
        );
        assert!(source.is_char_boundary(range.start()));
        assert!(source.is_char_boundary(range.end()));
        assert_eq!(&source[range.start()..range.end()], token.text());
    }

    for diagnostic in parse.diagnostics() {
        for label in diagnostic.labels() {
            let span = label.span();
            let range = span.range();
            assert_eq!(span.file(), TEST_FILE, "seed `{name}` changed FileId");
            assert!(
                range.end() <= source.len(),
                "seed `{name}` diagnostic range {range:?} exceeds {} bytes",
                source.len()
            );
            assert!(source.is_char_boundary(range.start()));
            assert!(source.is_char_boundary(range.end()));
        }
    }
}

fn significant_node_kinds(parse: &Parse) -> Vec<SyntaxKind> {
    parse
        .syntax()
        .descendants()
        .map(|node| node.kind())
        .collect()
}

fn significant_tokens(parse: &Parse) -> Vec<(SyntaxKind, String)> {
    parse
        .tokens()
        .iter()
        .filter(|token| !token.kind().is_trivia())
        .map(|token| (token.kind(), token.text().to_owned()))
        .collect()
}
