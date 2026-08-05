use super::{
    BLOCK_SIZE_EXPONENT, GameplayTag, GameplayTagBits, GameplayTagError, GameplayTagManager,
    MAX_TAG_BLOCKS, MAX_TAG_COUNTS, TAG_BITS_PER_BLOCK, tag_bits_from_tags,
};
use bevy::prelude::Component;

/// Reference-counted gameplay tags owned by one entity.
#[derive(Component)]
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
    /// Adds a tag and increments reference counts for it and all its parents.
    ///
    /// A single tag supports at most `u16::MAX` concurrent references on one
    /// entity. Debug builds report overflow while release builds saturate.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] if `tag` is not registered
    /// in `manager`.
    pub fn add_tag(
        &mut self,
        tag: &GameplayTag,
        manager: &GameplayTagManager,
    ) -> Result<(), GameplayTagError> {
        let inherited_bits = manager.get_inherited_bits(tag)?;
        for (block_index, &block_bits) in inherited_bits.iter().enumerate() {
            let base_index = block_index * TAG_BITS_PER_BLOCK;
            let mut current_block = block_bits;

            while current_block != 0 {
                let least_significant_bit = current_block & current_block.wrapping_neg();
                let bit_offset = least_significant_bit.trailing_zeros() as usize;
                let index = base_index + bit_offset;
                debug_assert!(index < self.ref_counts.len());
                let count = &mut self.ref_counts[index];
                debug_assert!(
                    *count < u16::MAX,
                    "gameplay tag reference count exceeded u16::MAX"
                );
                *count = count.saturating_add(1);
                current_block ^= least_significant_bit;
            }
        }

        for (destination, source) in self.tag_bits.iter_mut().zip(inherited_bits.iter()) {
            *destination |= *source;
        }
        Ok(())
    }

    /// Removes a tag and decrements references for it and all its parents.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] if `tag` is not registered
    /// in `manager`.
    pub fn remove_tag(
        &mut self,
        tag: &GameplayTag,
        manager: &GameplayTagManager,
    ) -> Result<(), GameplayTagError> {
        let tag_bit_index = tag.get_bit_index_usize();
        let inherited_bits = manager.get_inherited_bits(tag)?;
        if self.ref_counts[tag_bit_index] == 0 {
            return Ok(());
        }

        let mut bits_to_clear = [0u64; MAX_TAG_BLOCKS];
        for (block_index, &block_bits) in inherited_bits.iter().enumerate() {
            let base_index = block_index * TAG_BITS_PER_BLOCK;
            let mut current_block = block_bits;

            while current_block != 0 {
                let least_significant_bit = current_block & current_block.wrapping_neg();
                let bit_offset = least_significant_bit.trailing_zeros() as usize;
                let index = base_index + bit_offset;
                debug_assert!(index < self.ref_counts.len());
                let count = &mut self.ref_counts[index];
                *count = count.saturating_sub(1);
                if *count == 0 {
                    bits_to_clear[block_index] |= least_significant_bit;
                }
                current_block ^= least_significant_bit;
            }
        }

        for (destination, clear) in self.tag_bits.iter_mut().zip(bits_to_clear) {
            *destination &= !clear;
        }
        Ok(())
    }

    /// Adds multiple tags after validating all of them.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] before mutation if any tag
    /// is not registered in `manager`.
    pub fn add_tags(
        &mut self,
        tags: &[GameplayTag],
        manager: &GameplayTagManager,
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
        manager: &GameplayTagManager,
    ) -> Result<(), GameplayTagError> {
        for tag in tags {
            manager.get_inherited_bits(tag)?;
        }
        for tag in tags {
            self.remove_tag(tag, manager)?;
        }
        Ok(())
    }

    /// Returns whether `tag` or one of its explicitly counted references exists.
    pub fn has_tag(&self, tag: &GameplayTag) -> bool {
        let tag_bit_index = tag.get_bit_index_usize();
        if tag_bit_index >= MAX_TAG_COUNTS {
            return false;
        }
        let block = tag_bit_index >> BLOCK_SIZE_EXPONENT;
        let bit = tag_bit_index & (TAG_BITS_PER_BLOCK - 1);
        (self.tag_bits[block] & (1u64 << bit)) != 0
    }

    /// Returns whether all exact tag bits are present.
    pub fn has_all(&self, tags: &[GameplayTag]) -> bool {
        tag_bits_from_tags(tags).is_ok_and(|tag_bits| self.has_all_bits(&tag_bits))
    }

    /// Returns whether every bit in `tag_bits` is present.
    pub fn has_all_bits(&self, tag_bits: &GameplayTagBits) -> bool {
        self.tag_bits
            .iter()
            .zip(tag_bits.iter())
            .all(|(owned, required)| (owned & required) == *required)
    }

    /// Returns whether at least one exact tag bit is present.
    pub fn has_any(&self, tags: &[GameplayTag]) -> bool {
        tag_bits_from_tags(tags).is_ok_and(|tag_bits| self.has_any_bits(&tag_bits))
    }

    /// Returns whether at least one bit in `tag_bits` is present.
    pub fn has_any_bits(&self, tag_bits: &GameplayTagBits) -> bool {
        self.tag_bits
            .iter()
            .zip(tag_bits.iter())
            .any(|(owned, candidate)| (owned & candidate) != 0)
    }
}
