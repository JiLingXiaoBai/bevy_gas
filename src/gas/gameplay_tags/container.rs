use super::{
    BLOCK_SIZE_EXPONENT, GameplayTag, GameplayTagBits, GameplayTagError, GameplayTagManager,
    MAX_TAG_BLOCKS, MAX_TAG_COUNTS, TAG_BITS_PER_BLOCK, tag_bits_from_tags,
};
use bevy::prelude::Component;

#[derive(Clone, Copy, Default)]
struct TagReferenceCounts {
    explicit: u16,
    total: u16,
}

/// Reference-counted gameplay tags owned by one entity.
///
/// Explicit references track matching additions and removals. Total references also include
/// descendants and keep inherited tag bits present until their last contributing reference ends.
#[derive(Component)]
pub struct GameplayTagContainer {
    tag_bits: GameplayTagBits,
    ref_counts: Box<[TagReferenceCounts]>,
}

impl Default for GameplayTagContainer {
    fn default() -> Self {
        Self {
            tag_bits: GameplayTagBits::default(),
            ref_counts: Box::new([TagReferenceCounts::default(); MAX_TAG_COUNTS]),
        }
    }
}

impl GameplayTagContainer {
    /// Adds one explicit reference to `tag` and one total reference to it and all its parents.
    ///
    /// Each tag supports at most `u16::MAX` total references on one entity, including descendants.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] if `tag` is not registered in `manager`,
    /// or [`GameplayTagError::ReferenceCountOverflow`] if any affected total would exceed its
    /// capacity. Either error leaves the container unchanged.
    pub fn add_tag(
        &mut self,
        tag: &GameplayTag,
        manager: &GameplayTagManager,
    ) -> Result<(), GameplayTagError> {
        let inherited_bits = manager.get_inherited_bits(tag)?;
        for index in tag_bit_indices(inherited_bits) {
            self.validate_additional_references(index, 1)?;
        }

        // Every explicit reference also contributes to its tag's validated total.
        self.ref_counts[tag.get_bit_index_usize()].explicit += 1;
        for index in tag_bit_indices(inherited_bits) {
            self.ref_counts[index].total += 1;
        }
        for (destination, source) in self.tag_bits.iter_mut().zip(inherited_bits.iter()) {
            *destination |= *source;
        }
        Ok(())
    }

    /// Removes one explicit reference to `tag` and its contribution to all inherited totals.
    ///
    /// Does nothing when `tag` has no explicit references, even if descendants keep it present.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] if `tag` is not registered in `manager`.
    pub fn remove_tag(
        &mut self,
        tag: &GameplayTag,
        manager: &GameplayTagManager,
    ) -> Result<(), GameplayTagError> {
        let tag_bit_index = tag.get_bit_index_usize();
        let inherited_bits = manager.get_inherited_bits(tag)?;
        if self.ref_counts[tag_bit_index].explicit == 0 {
            return Ok(());
        }

        self.ref_counts[tag_bit_index].explicit -= 1;
        let mut bits_to_clear = [0u64; MAX_TAG_BLOCKS];
        for index in tag_bit_indices(inherited_bits) {
            let count = &mut self.ref_counts[index].total;
            debug_assert!(
                *count > 0,
                "an explicit reference must contribute to every ancestor"
            );
            *count -= 1;
            if *count == 0 {
                bits_to_clear[index / TAG_BITS_PER_BLOCK] |= 1u64 << (index % TAG_BITS_PER_BLOCK);
            }
        }

        for (destination, clear) in self.tag_bits.iter_mut().zip(bits_to_clear) {
            *destination &= !clear;
        }
        Ok(())
    }

    /// Adds one explicit reference per entry after validating the complete batch.
    ///
    /// Duplicate tags and shared ancestors each contribute to the batch's total reference counts.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] if any tag is not registered in `manager`,
    /// or [`GameplayTagError::ReferenceCountOverflow`] if the combined additions would exceed
    /// any tag's total capacity. Either error leaves the container unchanged.
    pub fn add_tags(
        &mut self,
        tags: &[GameplayTag],
        manager: &GameplayTagManager,
    ) -> Result<(), GameplayTagError> {
        let mut additional_references = [0u16; MAX_TAG_COUNTS];
        for tag in tags {
            for index in tag_bit_indices(manager.get_inherited_bits(tag)?) {
                additional_references[index] = additional_references[index].checked_add(1).ok_or(
                    GameplayTagError::ReferenceCountOverflow {
                        index,
                        max: u16::MAX,
                    },
                )?;
                self.validate_additional_references(index, additional_references[index])?;
            }
        }
        for tag in tags {
            self.add_tag(tag, manager)?;
        }
        Ok(())
    }

    /// Removes one explicit reference per entry after validating all tags.
    ///
    /// Entries without explicit references are ignored, including tags held only by inheritance.
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

    fn validate_additional_references(
        &self,
        index: usize,
        additional: u16,
    ) -> Result<(), GameplayTagError> {
        self.ref_counts[index]
            .total
            .checked_add(additional)
            .map(|_| ())
            .ok_or(GameplayTagError::ReferenceCountOverflow {
                index,
                max: u16::MAX,
            })
    }

    /// Returns whether `tag` has an explicit reference or inherits one from a descendant.
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

/// Visits set bits in block order, from the least significant bit in each block.
fn tag_bit_indices(bits: &GameplayTagBits) -> impl Iterator<Item = usize> + '_ {
    bits.iter()
        .enumerate()
        .flat_map(|(block_index, &block_bits)| {
            let mut remaining = block_bits;
            std::iter::from_fn(move || {
                if remaining == 0 {
                    return None;
                }
                let lowest_bit = remaining.isolate_lowest_one();
                remaining ^= lowest_bit;
                Some(block_index * TAG_BITS_PER_BLOCK + lowest_bit.trailing_zeros() as usize)
            })
        })
}
