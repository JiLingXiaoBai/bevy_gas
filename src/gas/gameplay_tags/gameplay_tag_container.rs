use super::{GameplayTag, GameplayTagManager, MAX_TAG_COUNTS};
use bevy::prelude::{Component, Res};

pub const BLOCK_SIZE_EXPONENT: usize = 6; // 2^6 =64
pub const TAG_BITS_PER_BLOCK: usize = 64;
pub const MAX_TAG_BLOCKS: usize = MAX_TAG_COUNTS.div_ceil(TAG_BITS_PER_BLOCK);

pub type GameplayTagBits = [u64; MAX_TAG_BLOCKS];

pub fn tag_bits_from_tags(tags: &[GameplayTag]) -> Option<GameplayTagBits> {
    let mut result = GameplayTagBits::default();
    for tag in tags {
        add_bit_with_tag(&mut result, tag)?;
    }
    Some(result)
}

pub fn tag_bits_from_tags_with_manager(
    tags: &[GameplayTag],
    manager: &Res<GameplayTagManager>,
) -> Option<GameplayTagBits> {
    let mut result = GameplayTagBits::default();
    for tag in tags {
        let inherited_bits = manager.get_inherited_bits(tag)?;
        for (dst, src) in result.iter_mut().zip(inherited_bits.iter()) {
            *dst |= *src;
        }
    }
    Some(result)
}

pub fn add_bit_with_tag(bits: &mut GameplayTagBits, tag: &GameplayTag) -> Option<()> {
    let tag_bit_index = tag.get_bit_index_usize();
    if tag_bit_index >= MAX_TAG_COUNTS {
        return None;
    }
    let block = tag_bit_index >> BLOCK_SIZE_EXPONENT;
    let bit = tag_bit_index & (TAG_BITS_PER_BLOCK - 1);
    bits[block] |= 1u64 << bit;
    Some(())
}

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
    /// Adds a tag, incrementing reference counts for itself and all parents, and updating the Bitset.
    pub fn add_tag(&mut self, tag: &GameplayTag, manager: &Res<GameplayTagManager>) {
        if let Some(inherited_bits) = manager.get_inherited_bits(tag) {
            // 1. Update Reference Counts (for self and all parents)
            for (block_index, &block_bits) in inherited_bits.iter().enumerate() {
                let base_index = (block_index * TAG_BITS_PER_BLOCK) as u16;
                let mut current_block = block_bits;

                while current_block != 0 {
                    let lsb = current_block & current_block.wrapping_neg();
                    let bit_offset = lsb.trailing_zeros();
                    let index_usize = base_index as usize + bit_offset as usize;
                    debug_assert!(index_usize < self.ref_counts.len());
                    self.ref_counts[index_usize] = self.ref_counts[index_usize].saturating_add(1);
                    current_block ^= lsb;
                }
            }

            // 2. Update Bitset (OR operation)
            for (dst, src) in self.tag_bits.iter_mut().zip(inherited_bits.iter()) {
                *dst |= *src;
            }
        }
    }
    /// Removes a tag, decrementing reference counts. Clears the bit only if the count drops to zero.
    pub fn remove_tag(&mut self, tag: &GameplayTag, manager: &Res<GameplayTagManager>) {
        let tag_bit_index = tag.get_bit_index_usize();
        if tag_bit_index < self.ref_counts.len()
            && self.ref_counts[tag_bit_index] > 0
            && let Some(inherited_bits) = manager.get_inherited_bits(tag)
        {
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
        }
    }

    pub fn add_tags(&mut self, tags: &[GameplayTag], manager: &Res<GameplayTagManager>) {
        for tag in tags {
            self.add_tag(tag, manager);
        }
    }

    pub fn remove_tags(&mut self, tags: &[GameplayTag], manager: &Res<GameplayTagManager>) {
        for tag in tags {
            self.remove_tag(tag, manager);
        }
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
        let Some(tag_bits) = tag_bits_from_tags(tags) else {
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
        let Some(tag_bits) = tag_bits_from_tags(tags) else {
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
