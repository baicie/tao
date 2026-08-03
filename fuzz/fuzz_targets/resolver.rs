#![no_main]

use std::cell::RefCell;

use libfuzzer_sys::fuzz_target;
use nexa_compiler::{
    CompilerInput, CompilerSource, FutaoResolverAdapter, ResolverAdapter,
    ResolverDifferentialHarness, ResolverDifferentialOutcome, ResolverAdapterError,
    RustResolverAdapter,
};

const MAX_SOURCE_BYTES: usize = 512;

thread_local! {
    static FUTAO: RefCell<Option<FutaoResolverAdapter>> =
        RefCell::new(FutaoResolverAdapter::new().ok());
}

fuzz_target!(|bytes: &[u8]| {
    if bytes.len() > MAX_SOURCE_BYTES {
        return;
    }
    let Ok(source) = std::str::from_utf8(bytes) else {
        return;
    };
    let input = CompilerInput::new(
        "main.ft",
        [CompilerSource::new("main.ft", source)],
    );

    FUTAO.with(|slot| {
        let adapter = slot.borrow();
        let Some(futao) = adapter.as_ref() else {
            panic!("Futao resolver adapter failed to initialize");
        };
        let rust = RustResolverAdapter;
        let reference = match rust.resolve(&input) {
            Ok(snapshot) => snapshot,
            Err(ResolverAdapterError::InvalidInput(_)) => return,
            Err(error) => panic!("Rust resolver adapter failed: {error}"),
        };
        let harness = ResolverDifferentialHarness::new(&rust, futao);
        let report = harness
            .run_case("fuzz/generated", &input)
            .unwrap_or_else(|error| panic!("resolver adapter failed: {error}"));
        assert_eq!(report.outcome(), ResolverDifferentialOutcome::Match);
        assert_eq!(report.reference_snapshot(), &reference);
    });
});
