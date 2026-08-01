#![no_main]

use libfuzzer_sys::fuzz_target;
use nexa_parser::parse_source;
use nexa_span::FileId;

const TEST_FILE: FileId = FileId::new(0);

fuzz_target!(|bytes: &[u8]| {
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
});
