//! Deterministic bounded collection acceptance/rejection coverage.

use nexa_storage::{
    BitSet, BitSetError, DeterministicMap, DeterministicSet, InternError, InternTable,
};

#[test]
fn deterministic_map_iterates_in_key_order_and_replacement_does_not_consume_capacity(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut values = DeterministicMap::new(3);
    values.try_insert(String::from("zeta"), 1)?;
    values.try_insert(String::from("alpha"), 2)?;
    values.try_insert(String::from("middle"), 3)?;
    assert_eq!(values.try_insert(String::from("alpha"), 4)?, Some(2));

    assert_eq!(
        values
            .iter()
            .map(|(key, value)| (key.as_str(), *value))
            .collect::<Vec<_>>(),
        [("alpha", 4), ("middle", 3), ("zeta", 1)]
    );
    Ok(())
}

#[test]
fn deterministic_map_capacity_error_returns_the_owned_entry(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut values = DeterministicMap::new(1);
    values.try_insert(String::from("first"), 1)?;
    let error = values
        .try_insert(String::from("second"), 2)
        .err()
        .ok_or("expected bounded map capacity failure")?;

    assert_eq!(error.into_entry(), (String::from("second"), 2));
    Ok(())
}

#[test]
fn deterministic_set_and_bit_set_iterate_in_ascending_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut names = DeterministicSet::new(3);
    names.try_insert(String::from("z"))?;
    names.try_insert(String::from("a"))?;
    names.try_insert(String::from("m"))?;
    let mut bits = BitSet::new(130);
    bits.insert(129)?;
    bits.insert(1)?;
    bits.insert(64)?;

    assert_eq!(
        names.iter().map(String::as_str).collect::<Vec<_>>(),
        ["a", "m", "z"]
    );
    assert_eq!(bits.iter().collect::<Vec<_>>(), [1, 64, 129]);
    Ok(())
}

#[test]
fn intern_table_reuses_ids_and_preserves_first_intern_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut symbols = InternTable::new(3);
    let zeta = symbols.intern("zeta")?;
    let alpha = symbols.intern("alpha")?;
    let repeated = symbols.intern("zeta")?;

    assert_eq!(zeta.index(), 0);
    assert_eq!(alpha.index(), 1);
    assert_eq!(repeated, zeta);
    assert_eq!(symbols.resolve(alpha), Some("alpha"));
    Ok(())
}

#[test]
fn bit_set_rejects_its_exclusive_upper_bound() {
    let mut bits = BitSet::new(64);

    assert_eq!(
        bits.insert(64),
        Err(BitSetError::OutOfRange {
            index: 64,
            limit: 64
        })
    );
}

#[test]
fn intern_table_rejects_a_new_symbol_at_capacity_but_reuses_existing_ids(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut symbols = InternTable::new(1);
    let first = symbols.intern("first")?;

    assert_eq!(symbols.intern("first")?, first);
    assert_eq!(
        symbols.intern("second"),
        Err(InternError::CapacityExceeded { limit: 1 })
    );
    Ok(())
}
