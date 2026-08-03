use bevy::platform::collections::HashMap;
use bevy::platform::hash::FixedHasher;
use bevy::prelude::*;
use core::fmt;
use core::hash::{BuildHasher, Hash};
use std::error::Error;

fn compute_hash(input: &str) -> u64 {
    FixedHasher.hash_one(input)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct UniqueName(u32);

impl fmt::Debug for UniqueName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UniqueName({})", self.0)
    }
}

/// Error returned when a unique name cannot be interned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniqueNameError {
    /// The `u32` handle space is exhausted.
    CapacityExceeded { max: u64 },
}

impl fmt::Display for UniqueNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapacityExceeded { max } => {
                write!(f, "unique name capacity exceeded; max names: {max}")
            }
        }
    }
}

impl Error for UniqueNameError {}

#[derive(Debug)]
enum HashBucket {
    Single(u32),
    Collisions(Vec<u32>),
}

impl HashBucket {
    fn find(&self, entry_pool: &[String], name: &str) -> Option<u32> {
        match self {
            Self::Single(index) => entry_matches(entry_pool, *index, name).then_some(*index),
            Self::Collisions(indices) => indices
                .iter()
                .copied()
                .find(|&index| entry_matches(entry_pool, index, name)),
        }
    }

    fn push(&mut self, index: u32) {
        match self {
            Self::Single(existing) => {
                *self = Self::Collisions(vec![*existing, index]);
            }
            Self::Collisions(indices) => indices.push(index),
        }
    }
}

fn entry_matches(entry_pool: &[String], index: u32, name: &str) -> bool {
    entry_pool
        .get(index as usize)
        .is_some_and(|entry| entry == name)
}

#[derive(Resource)]
pub struct UniqueNamePool {
    entry_pool: Vec<String>,
    lookup_hash: HashMap<u64, HashBucket>,
}
impl Default for UniqueNamePool {
    fn default() -> Self {
        let mut pool = Self {
            entry_pool: Vec::new(),
            lookup_hash: HashMap::new(),
        };
        pool.entry_pool.push("".to_string());
        pool
    }
}

impl UniqueNamePool {
    const MAX_NAMES: u64 = u32::MAX as u64 + 1;

    fn get_or_insert(&mut self, name: &str) -> Result<u32, UniqueNameError> {
        if name.is_empty() {
            return Ok(0);
        }

        let hash = compute_hash(name);
        if let Some(index) = self
            .lookup_hash
            .get(&hash)
            .and_then(|bucket| bucket.find(&self.entry_pool, name))
        {
            return Ok(index);
        }

        let new_index = u32::try_from(self.entry_pool.len()).map_err(|_| {
            UniqueNameError::CapacityExceeded {
                max: Self::MAX_NAMES,
            }
        })?;
        self.entry_pool.push(name.to_string());
        self.lookup_hash
            .entry(hash)
            .and_modify(|bucket| bucket.push(new_index))
            .or_insert(HashBucket::Single(new_index));
        Ok(new_index)
    }

    /// Returns the existing handle for `name` or interns it.
    ///
    /// Hash collisions are resolved by comparing the complete strings. An error
    /// is returned only when every value representable by the `u32` handle has
    /// been allocated.
    pub fn new_name(&mut self, name: &str) -> Result<UniqueName, UniqueNameError> {
        self.get_or_insert(name).map(UniqueName)
    }

    /// Returns the interned string for `name`, or an empty string for a stale handle.
    pub fn get_display_str(&self, name: &UniqueName) -> &str {
        self.entry_pool
            .get(name.0 as usize)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// Removes all interned names while preserving the empty-name handle.
    pub fn clear(&mut self) {
        self.lookup_hash.clear();
        self.entry_pool.truncate(1); // only keep the first element: ""
    }
}
