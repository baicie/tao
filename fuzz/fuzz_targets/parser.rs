#![no_main]

use libfuzzer_sys::fuzz_target;
use nexa_parser::parse_source;
use nexa_span::FileId;

fuzz_target!(|bytes: &[u8]| {
    let Ok(source) = std::str::from_utf8(bytes) else {
        return;
    };

    let first = parse_source(FileId::new(0), source);
    let second = parse_source(FileId::new(0), source);

    assert_eq!(first.syntax().to_string(), source);
    assert_eq!(first.tokens(), second.tokens());
    assert_eq!(first.diagnostics(), second.diagnostics());
    assert_eq!(first.debug_tree(), second.debug_tree());
});
