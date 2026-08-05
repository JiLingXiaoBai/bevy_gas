use super::{GameplayTag, GameplayTagError};
use crate::settings::GameplayAbilitySystemSettings;

/// Maximum number of gameplay tags that can be registered.
pub const MAX_TAG_COUNTS: usize = GameplayAbilitySystemSettings::GAMEPLAY_TAG_SIZE;
/// Exponent used to divide a tag index by the 64-bit block size.
pub const BLOCK_SIZE_EXPONENT: usize = 6;
/// Number of gameplay-tag bits stored in one block.
pub const TAG_BITS_PER_BLOCK: usize = 64;
/// Number of blocks needed for the configured tag capacity.
pub const MAX_TAG_BLOCKS: usize = MAX_TAG_COUNTS.div_ceil(TAG_BITS_PER_BLOCK);

/// Fixed-size bitset used for fast gameplay-tag matching.
pub type GameplayTagBits = [u64; MAX_TAG_BLOCKS];

/// Builds a bitset containing exactly the supplied tags.
///
/// # Errors
///
/// Returns [`GameplayTagError::InvalidTagIndex`] if a tag index exceeds the
/// configured gameplay-tag capacity.
pub fn tag_bits_from_tags(tags: &[GameplayTag]) -> Result<GameplayTagBits, GameplayTagError> {
    let mut result = GameplayTagBits::default();
    for tag in tags {
        add_bit_with_tag(&mut result, tag)?;
    }
    Ok(result)
}

/// Sets the bit corresponding to `tag` in `bits`.
///
/// # Errors
///
/// Returns [`GameplayTagError::InvalidTagIndex`] if the tag index exceeds the
/// configured gameplay-tag capacity.
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
