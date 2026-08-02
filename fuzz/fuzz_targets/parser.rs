#![no_main]

use std::cell::RefCell;

use libfuzzer_sys::fuzz_target;
use nexa_compiler::{
    FutaoParserAdapter, ParserDifferentialHarness, ParserDifferentialOutcome, RustParserAdapter,
};
use nexa_parser::parse_source;
use nexa_span::FileId;

const MAX_SOURCE_BYTES: usize = 512;
const TEST_FILE: FileId = FileId::new(0);

thread_local! {
    static FUTAO: RefCell<Option<FutaoParserAdapter>> =
        RefCell::new(FutaoParserAdapter::new().ok());
}

fuzz_target!(|bytes: &[u8]| {
    if bytes.len() > MAX_SOURCE_BYTES {
        return;
    }
    let Ok(source) = std::str::from_utf8(bytes) else {
        return;
    };

    let first = parse_source(TEST_FILE, source);
    let second = parse_source(TEST_FILE, source);

    assert_eq!(first.syntax().to_string(), source);
    assert_eq!(first.tokens(), second.tokens());
    assert_eq!(first.diagnostics(), second.diagnostics());
    assert_eq!(first.debug_tree(), second.debug_tree());

    let mut token_cursor = 0;
    for token in first.tokens() {
        let range = token.range();
        assert_eq!(range.start(), token_cursor);
        assert!(range.start() <= range.end());
        assert!(range.end() <= source.len());
        assert!(source.is_char_boundary(range.start()));
        assert!(source.is_char_boundary(range.end()));
        assert_eq!(&source[range.start()..range.end()], token.text());
        token_cursor = range.end();
    }
    assert_eq!(token_cursor, source.len());

    let mut previous_label_range = None;
    for diagnostic in first.diagnostics() {
        for label in diagnostic.labels() {
            let span = label.span();
            let range = span.range();
            let range_key = (range.start(), range.end());

            assert_eq!(span.file(), TEST_FILE);
            assert!(range.start() <= range.end());
            assert!(range.end() <= source.len());
            assert!(source.is_char_boundary(range.start()));
            assert!(source.is_char_boundary(range.end()));
            if let Some(previous) = previous_label_range {
                assert!(previous <= range_key);
            }
            previous_label_range = Some(range_key);
        }
    }

    FUTAO.with(|slot| {
        let adapter = slot.borrow();
        let Some(futao) = adapter.as_ref() else {
            panic!("Futao parser adapter failed to initialize");
        };
        let rust = RustParserAdapter;
        let harness = ParserDifferentialHarness::new(&rust, futao);
        let report = harness
            .run_case("fuzz/generated", source)
            .unwrap_or_else(|error| panic!("parser adapter failed: {error}"));
        assert_eq!(report.outcome(), ParserDifferentialOutcome::Match);
    });
});
