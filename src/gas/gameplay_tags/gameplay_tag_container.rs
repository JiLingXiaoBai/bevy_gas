use super::{GameplayTag, GameplayTagError, GameplayTagManager, MAX_TAG_COUNTS};
use crate::gameplay_effects::ActiveGameplayEffects;
use bevy::prelude::{Component, Res};

pub const BLOCK_SIZE_EXPONENT: usize = 6; // 2^6 =64
pub const TAG_BITS_PER_BLOCK: usize = 64;
pub const MAX_TAG_BLOCKS: usize = MAX_TAG_COUNTS.div_ceil(TAG_BITS_PER_BLOCK);

pub type GameplayTagBits = [u64; MAX_TAG_BLOCKS];

/// Builds a bitset containing exactly the supplied tags.
///
/// # Arguments
///
/// * `tags` - Tags whose exact bits will be set.
///
/// # Errors
///
/// Returns [`GameplayTagError::InvalidTagIndex`] if a tag index exceeds the
/// configured gameplay tag capacity.
pub fn tag_bits_from_tags(tags: &[GameplayTag]) -> Result<GameplayTagBits, GameplayTagError> {
    let mut result = GameplayTagBits::default();
    for tag in tags {
        add_bit_with_tag(&mut result, tag)?;
    }
    Ok(result)
}

/// Builds a bitset containing the supplied tags and all their parent tags.
///
/// # Arguments
///
/// * `tags` - Tags whose inherited bitsets will be combined.
/// * `manager` - Tag manager that provides the inherited bitsets.
///
/// # Errors
///
/// Returns [`GameplayTagError::InvalidTagIndex`] if a tag is not registered in
/// the supplied manager.
pub fn tag_bits_from_tags_with_manager(
    tags: &[GameplayTag],
    manager: &Res<GameplayTagManager>,
) -> Result<GameplayTagBits, GameplayTagError> {
    let mut result = GameplayTagBits::default();
    for tag in tags {
        let inherited_bits = manager.get_inherited_bits(tag)?;
        for (dst, src) in result.iter_mut().zip(inherited_bits.iter()) {
            *dst |= *src;
        }
    }
    Ok(result)
}

/// Sets the bit corresponding to `tag` in `bits`.
///
/// # Arguments
///
/// * `bits` - Bitset to update.
/// * `tag` - Tag whose bit will be set.
///
/// # Errors
///
/// Returns [`GameplayTagError::InvalidTagIndex`] if the tag index exceeds the
/// configured gameplay tag capacity.
pub fn add_bit_with_tag(
    bits: &mut GameplayTagBits,
    tag: &GameplayTag,
) -> Result<(), GameplayTagError> {
    let tag_bit_index = tag.get_bit_index_usize();
    if tag_bit_index >= MAX_TAG_COUNTS {
        return Err(GameplayTagError::InvalidTagIndex {
            index: tag_bit_index,
        });
    }
    let block = tag_bit_index >> BLOCK_SIZE_EXPONENT;
    let bit = tag_bit_index & (TAG_BITS_PER_BLOCK - 1);
    bits[block] |= 1u64 << bit;
    Ok(())
}

#[derive(Component)]
#[require(ActiveGameplayEffects)]
pub struct GameplayTagContainer {
    tag_bits: GameplayTagBits,
    ref_counts: Box<[u16]>,
}

impl Default for GameplayTagContainer {
    fn default() -> Self {
        Self {
            tag_bits: GameplayTagBits::default(),
            ref_counts: Box::new([0; MAX_TAG_COUNTS]),
        }
    }
}

impl GameplayTagContainer {
    /// Adds a tag, incrementing reference counts for itself and all parents, and updating the Bitset.
    ///
    /// A single tag supports at most `u16::MAX` concurrent references on one
    /// entity. Exceeding this limit violates an internal invariant: debug builds
    /// report it with `debug_assert!`, while release builds keep the count
    /// saturated to avoid wrapping or panicking.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] if `tag` is not registered
    /// in `manager`.
    pub fn add_tag(
        &mut self,
        tag: &GameplayTag,
        manager: &Res<GameplayTagManager>,
    ) -> Result<(), GameplayTagError> {
        let inherited_bits = manager.get_inherited_bits(tag)?;
        // 1. Update Reference Counts (for self and all parents)
        for (block_index, &block_bits) in inherited_bits.iter().enumerate() {
            let base_index = (block_index * TAG_BITS_PER_BLOCK) as u16;
            let mut current_block = block_bits;

            while current_block != 0 {
                let lsb = current_block & current_block.wrapping_neg();
                let bit_offset = lsb.trailing_zeros();
                let index_usize = base_index as usize + bit_offset as usize;
                debug_assert!(index_usize < self.ref_counts.len());
                let count = &mut self.ref_counts[index_usize];
                debug_assert!(
                    *count < u16::MAX,
                    "gameplay tag reference count exceeded u16::MAX"
                );
                *count = count.saturating_add(1);
                current_block ^= lsb;
            }
        }

        // 2. Update Bitset (OR operation)
        for (dst, src) in self.tag_bits.iter_mut().zip(inherited_bits.iter()) {
            *dst |= *src;
        }
        Ok(())
    }
    /// Removes a tag, decrementing reference counts. Clears the bit only if the count drops to zero.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] if `tag` is not registered
    /// in `manager`.
    pub fn remove_tag(
        &mut self,
        tag: &GameplayTag,
        manager: &Res<GameplayTagManager>,
    ) -> Result<(), GameplayTagError> {
        let tag_bit_index = tag.get_bit_index_usize();
        let inherited_bits = manager.get_inherited_bits(tag)?;
        if self.ref_counts[tag_bit_index] == 0 {
            return Ok(());
        }

        // 1. Update Reference Counts and track which bits need to be cleared
        let mut bits_to_clear = [0u64; MAX_TAG_BLOCKS];
        for (block_index, &block_bits) in inherited_bits.iter().enumerate() {
            let base_index = (block_index * TAG_BITS_PER_BLOCK) as u16;
            let mut current_block = block_bits;

            while current_block != 0 {
                let lsb = current_block & current_block.wrapping_neg();
                let bit_offset = lsb.trailing_zeros();
                let index_usize = base_index as usize + bit_offset as usize;
                debug_assert!(index_usize < self.ref_counts.len());
                let cnt = &mut self.ref_counts[index_usize];
                *cnt = cnt.saturating_sub(1);
                if *cnt == 0 {
                    bits_to_clear[block_index] |= lsb;
                }
                current_block ^= lsb;
            }
        }

        // 2. Update Bitset (AND NOT operation based on zero counts)
        for (dst, clear) in &mut self.tag_bits.iter_mut().zip(bits_to_clear) {
            *dst &= !clear;
        }
        Ok(())
    }

    /// Adds multiple tags with the same per-tag reference limit as [`Self::add_tag`].
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] before mutation if any tag
    /// is not registered in `manager`.
    pub fn add_tags(
        &mut self,
        tags: &[GameplayTag],
        manager: &Res<GameplayTagManager>,
    ) -> Result<(), GameplayTagError> {
        for tag in tags {
            manager.get_inherited_bits(tag)?;
        }
        for tag in tags {
            self.add_tag(tag, manager)?;
        }
        Ok(())
    }

    /// Removes multiple tags after validating all of them.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] before mutation if any tag
    /// is not registered in `manager`.
    pub fn remove_tags(
        &mut self,
        tags: &[GameplayTag],
        manager: &Res<GameplayTagManager>,
    ) -> Result<(), GameplayTagError> {
        for tag in tags {
            manager.get_inherited_bits(tag)?;
        }
        for tag in tags {
            self.remove_tag(tag, manager)?;
        }
        Ok(())
    }
    pub fn has_tag(&self, tag: &GameplayTag) -> bool {
        let tag_bit_index = tag.get_bit_index_usize();
        if tag_bit_index >= MAX_TAG_COUNTS {
            return false;
        };
        let block = tag_bit_index >> BLOCK_SIZE_EXPONENT;
        let bit = tag_bit_index & (TAG_BITS_PER_BLOCK - 1);
        (self.tag_bits[block] & (1u64 << bit)) != 0
    }
    pub fn has_all(&self, tags: &[GameplayTag]) -> bool {
        let Ok(tag_bits) = tag_bits_from_tags(tags) else {
            return false;
        };
        self.has_all_bits(&tag_bits)
    }

    pub fn has_all_bits(&self, tag_bits: &GameplayTagBits) -> bool {
        self.tag_bits
            .iter()
            .zip(tag_bits.iter())
            .all(|(a, b)| (a & b) == *b)
    }

    pub fn has_any(&self, tags: &[GameplayTag]) -> bool {
        let Ok(tag_bits) = tag_bits_from_tags(tags) else {
            return false;
        };
        self.has_any_bits(&tag_bits)
    }

    pub fn has_any_bits(&self, tag_bits: &GameplayTagBits) -> bool {
        self.tag_bits
            .iter()
            .zip(tag_bits.iter())
            .any(|(a, b)| (a & b) != 0)
    }
}
