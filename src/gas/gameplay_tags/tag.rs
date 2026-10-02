/// Compact identifier assigned to a registered gameplay tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameplayTag(u16);

impl GameplayTag {
    pub(crate) const fn new(tag_bit_index: u16) -> Self {
        Self(tag_bit_index)
    }

    /// Returns the bit index as `u16`.
    pub const fn get_bit_index_u16(&self) -> u16 {
        self.0
    }

    /// Returns the bit index as `usize`.
    pub const fn get_bit_index_usize(&self) -> usize {
        self.0 as usize
    }
}
