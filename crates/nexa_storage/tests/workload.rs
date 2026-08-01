//! Release-mode compiler-storage workload baseline.

use std::time::Instant;

use nexa_storage::{DeterministicMap, DeterministicSet, InternTable};

const ENTRY_COUNT: usize = 20_000;

#[test]
#[ignore = "run explicitly through `cargo xtask storage-kernel`"]
fn compiler_collection_workload_preserves_bounds_order_and_symbol_identity(
) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let mut map = DeterministicMap::new(ENTRY_COUNT);
    let mut set = DeterministicSet::new(ENTRY_COUNT);
    let mut symbols = InternTable::new(ENTRY_COUNT);

    for index in (0..ENTRY_COUNT).rev() {
        map.try_insert(index, index * 2)?;
        set.try_insert(index)?;
        let text = format!("symbol_{index:05}");
        let symbol = symbols.intern(&text)?;
        assert_eq!(symbols.resolve(symbol), Some(text.as_str()));
    }

    assert_eq!(map.len(), ENTRY_COUNT);
    assert_eq!(map.iter().next(), Some((&0, &0)));
    assert_eq!(
        map.iter().next_back().map(|(key, value)| (*key, *value)),
        Some((ENTRY_COUNT - 1, (ENTRY_COUNT - 1) * 2))
    );
    assert_eq!(set.iter().next(), Some(&0));
    assert_eq!(set.iter().next_back(), Some(&(ENTRY_COUNT - 1)));
    eprintln!(
        "nexa-storage workload entries={ENTRY_COUNT} elapsed_us={}",
        started.elapsed().as_micros()
    );
    Ok(())
}
