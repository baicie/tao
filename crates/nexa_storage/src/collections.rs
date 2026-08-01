use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;

use thiserror::Error;

/// A bounded ordered map with stable key-order iteration.
#[derive(Debug, Clone)]
pub struct DeterministicMap<K, V> {
    entries: BTreeMap<K, V>,
    max_entries: usize,
}

impl<K: Ord, V> DeterministicMap<K, V> {
    /// Creates an empty map with an explicit distinct-key bound.
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            max_entries,
        }
    }

    /// Returns the number of distinct keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true when the map has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Inserts or replaces an entry while preserving key-order iteration.
    ///
    /// # Errors
    ///
    /// Returns [`MapCapacityError`] with the unconsumed entry if a new key
    /// exceeds the logical bound. Physical allocation follows the Application
    /// Profile's Host OOM policy.
    pub fn try_insert(&mut self, key: K, value: V) -> Result<Option<V>, MapCapacityError<K, V>> {
        if self.entries.len() >= self.max_entries && !self.entries.contains_key(&key) {
            return Err(MapCapacityError {
                limit: self.max_entries,
                key,
                value,
            });
        }
        Ok(self.entries.insert(key, value))
    }

    /// Looks up a value by key.
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key)
    }

    /// Iterates entries in ascending key order.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator {
        self.entries.iter()
    }
}

/// Failed map insertion that retains ownership of the rejected entry.
#[derive(Debug)]
pub struct MapCapacityError<K, V> {
    limit: usize,
    key: K,
    value: V,
}

impl<K, V> MapCapacityError<K, V> {
    /// Returns ownership of the entry that was never inserted.
    #[must_use]
    pub fn into_entry(self) -> (K, V) {
        (self.key, self.value)
    }
}

impl<K, V> Display for MapCapacityError<K, V> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "map entry limit {} exceeded", self.limit)
    }
}

impl<K: Debug, V: Debug> std::error::Error for MapCapacityError<K, V> {}

/// A bounded ordered set with stable value-order iteration.
#[derive(Debug, Clone)]
pub struct DeterministicSet<T> {
    values: BTreeSet<T>,
    max_values: usize,
}

impl<T: Ord> DeterministicSet<T> {
    /// Creates an empty set with an explicit distinct-value bound.
    #[must_use]
    pub fn new(max_values: usize) -> Self {
        Self {
            values: BTreeSet::new(),
            max_values,
        }
    }

    /// Inserts a value, returning whether it was newly present.
    ///
    /// # Errors
    ///
    /// Returns [`SetCapacityError`] with the unconsumed value if the logical
    /// bound rejects a distinct value. Physical allocation follows the
    /// Application Profile's Host OOM policy.
    pub fn try_insert(&mut self, value: T) -> Result<bool, SetCapacityError<T>> {
        if self.values.contains(&value) {
            return Ok(false);
        }
        if self.values.len() >= self.max_values {
            return Err(SetCapacityError {
                limit: self.max_values,
                value,
            });
        }
        Ok(self.values.insert(value))
    }

    /// Returns whether the set contains `value`.
    #[must_use]
    pub fn contains(&self, value: &T) -> bool {
        self.values.contains(value)
    }

    /// Iterates values in ascending order.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.values.iter()
    }
}

/// Failed set insertion that retains ownership of the rejected value.
#[derive(Debug)]
pub struct SetCapacityError<T> {
    limit: usize,
    value: T,
}

impl<T> SetCapacityError<T> {
    /// Returns ownership of the value that was never inserted.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }
}

impl<T> Display for SetCapacityError<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "set value limit {} exceeded", self.limit)
    }
}

impl<T: Debug> std::error::Error for SetCapacityError<T> {}

/// A bounded set of non-negative integer indexes.
#[derive(Debug, Clone)]
pub struct BitSet {
    words: Vec<u64>,
    max_bits: usize,
}

impl BitSet {
    /// Creates an empty bit set with indexes in `0..max_bits`.
    #[must_use]
    pub const fn new(max_bits: usize) -> Self {
        Self {
            words: Vec::new(),
            max_bits,
        }
    }

    /// Adds one bounded index.
    ///
    /// # Errors
    ///
    /// Returns [`BitSetError`] if the index is out of range or backing storage
    /// cannot grow.
    pub fn insert(&mut self, index: usize) -> Result<bool, BitSetError> {
        if index >= self.max_bits {
            return Err(BitSetError::OutOfRange {
                index,
                limit: self.max_bits,
            });
        }
        let word_index = index / u64::BITS as usize;
        if word_index >= self.words.len() {
            let required = word_index + 1;
            self.words
                .try_reserve_exact(required - self.words.len())
                .map_err(|_| BitSetError::HostAllocationFailed)?;
            self.words.resize(required, 0);
        }
        let mask = 1_u64 << (index % u64::BITS as usize);
        let was_present = self.words[word_index] & mask != 0;
        self.words[word_index] |= mask;
        Ok(!was_present)
    }

    /// Returns whether an in-range index is present.
    #[must_use]
    pub fn contains(&self, index: usize) -> bool {
        if index >= self.max_bits {
            return false;
        }
        let word_index = index / u64::BITS as usize;
        self.words
            .get(word_index)
            .is_some_and(|word| word & (1_u64 << (index % u64::BITS as usize)) != 0)
    }

    /// Iterates present indexes in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.words
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(word_index, word)| {
                (0..u64::BITS as usize).filter_map(move |bit_index| {
                    let mask = 1_u64 << bit_index;
                    (word & mask != 0).then_some(word_index * u64::BITS as usize + bit_index)
                })
            })
            .take_while(|index| *index < self.max_bits)
    }
}

/// Failure from a bounded bit-set operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BitSetError {
    /// The requested index is outside the configured bit range.
    #[error("bit index {index} is outside limit {limit}")]
    OutOfRange {
        /// Rejected bit index.
        index: usize,
        /// Exclusive upper bound.
        limit: usize,
    },
    /// The Host could not reserve backing words.
    #[error("Host could not reserve bit-set storage")]
    HostAllocationFailed,
}

/// Stable identifier assigned by [`InternTable`] in first-intern order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(u32);

impl SymbolId {
    /// Returns the zero-based first-intern index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

/// A bounded string intern table with deterministic symbol assignment.
#[derive(Debug, Clone)]
pub struct InternTable {
    ids: BTreeMap<Rc<str>, SymbolId>,
    values: Vec<Rc<str>>,
    max_symbols: usize,
}

impl InternTable {
    /// Creates an empty table with an explicit distinct-symbol bound.
    #[must_use]
    pub fn new(max_symbols: usize) -> Self {
        Self {
            ids: BTreeMap::new(),
            values: Vec::new(),
            max_symbols,
        }
    }

    /// Interns UTF-8 text, preserving the first assigned identifier.
    ///
    /// # Errors
    ///
    /// Returns [`InternError`] when the symbol bound or identifier space rejects
    /// a new distinct value. Physical allocation follows the Application
    /// Profile's Host OOM policy.
    pub fn intern(&mut self, value: &str) -> Result<SymbolId, InternError> {
        if let Some(symbol) = self.ids.get(value) {
            return Ok(*symbol);
        }
        if self.values.len() >= self.max_symbols {
            return Err(InternError::CapacityExceeded {
                limit: self.max_symbols,
            });
        }
        let index =
            u32::try_from(self.values.len()).map_err(|_| InternError::SymbolSpaceExhausted)?;
        let symbol = SymbolId(index);
        let owned: Rc<str> = Rc::from(value);
        self.ids.insert(Rc::clone(&owned), symbol);
        self.values.push(owned);
        Ok(symbol)
    }

    /// Resolves a symbol identifier to its interned UTF-8 text.
    #[must_use]
    pub fn resolve(&self, symbol: SymbolId) -> Option<&str> {
        usize::try_from(symbol.0)
            .ok()
            .and_then(|index| self.values.get(index))
            .map(Rc::as_ref)
    }
}

/// Failure while interning a new distinct string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InternError {
    /// The configured distinct-symbol bound was reached.
    #[error("intern table symbol limit {limit} exceeded")]
    CapacityExceeded {
        /// Configured symbol bound.
        limit: usize,
    },
    /// The stable `u32` symbol identifier space was exhausted.
    #[error("intern table symbol identifier space exhausted")]
    SymbolSpaceExhausted,
}

/// Deterministic UTF-8-keyed map.
pub type StringMap<V> = DeterministicMap<String, V>;
/// Deterministic integer-keyed map.
pub type IntMap<V> = DeterministicMap<u64, V>;
/// Deterministic UTF-8 value set.
pub type StringSet = DeterministicSet<String>;
/// Deterministic integer value set.
pub type IntSet = DeterministicSet<u64>;
