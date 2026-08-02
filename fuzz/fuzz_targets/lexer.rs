#![no_main]

use std::cell::RefCell;

use libfuzzer_sys::fuzz_target;
use nexa_compiler::{
    FutaoLexerAdapter, LexerDifferentialHarness, LexerDifferentialOutcome, RustLexerAdapter,
};

const MAX_SOURCE_BYTES: usize = 512;

thread_local! {
    static FUTAO: RefCell<Option<FutaoLexerAdapter>> =
        RefCell::new(FutaoLexerAdapter::new().ok());
}

fuzz_target!(|bytes: &[u8]| {
    if bytes.len() > MAX_SOURCE_BYTES {
        return;
    }
    let Ok(source) = std::str::from_utf8(bytes) else {
        return;
    };

    FUTAO.with(|slot| {
        let adapter = slot.borrow();
        let Some(futao) = adapter.as_ref() else {
            panic!("Futao lexer adapter failed to initialize");
        };
        let rust = RustLexerAdapter;
        let harness = LexerDifferentialHarness::new(&rust, futao);
        let report = harness
            .run_case("fuzz/generated", source)
            .unwrap_or_else(|error| panic!("lexer adapter failed: {error}"));
        assert_eq!(report.outcome(), LexerDifferentialOutcome::Match);
    });
});
