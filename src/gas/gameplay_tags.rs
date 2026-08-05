//! Hierarchical gameplay tags, registration, bitsets, and reference-counted storage.

mod bitset;
mod container;
mod registry;
mod requirements;
mod tag;

pub use bitset::{
    BLOCK_SIZE_EXPONENT, GameplayTagBits, MAX_TAG_BLOCKS, MAX_TAG_COUNTS, TAG_BITS_PER_BLOCK,
    add_bit_with_tag, tag_bits_from_tags,
};
pub use container::GameplayTagContainer;
pub use registry::{GameplayTagManager, GameplayTagRegister, tag_bits_from_tags_with_manager};
pub use requirements::TagRequirements;
pub use tag::{GameplayTag, GameplayTagError};
