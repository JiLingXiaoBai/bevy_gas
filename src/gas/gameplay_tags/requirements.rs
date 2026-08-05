use super::{
    GameplayTag, GameplayTagBits, GameplayTagContainer, GameplayTagError, GameplayTagManager,
    tag_bits_from_tags, tag_bits_from_tags_with_manager,
};

/// Required and blocked gameplay tags with precomputed matching bitsets.
#[derive(Clone, Default)]
pub struct TagRequirements {
    require_all: Vec<GameplayTag>,
    ignore_any: Vec<GameplayTag>,
    require_all_bits: GameplayTagBits,
    ignore_any_bits: GameplayTagBits,
}

impl TagRequirements {
    /// Creates and precomputes a set of required and ignored tags.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] if any tag index exceeds
    /// the configured capacity.
    pub fn new(
        require_all: Vec<GameplayTag>,
        ignore_any: Vec<GameplayTag>,
    ) -> Result<Self, GameplayTagError> {
        let require_all_bits = tag_bits_from_tags(&require_all)?;
        let ignore_any_bits = tag_bits_from_tags(&ignore_any)?;
        Ok(Self {
            require_all,
            ignore_any,
            require_all_bits,
            ignore_any_bits,
        })
    }

    /// Returns whether no required or ignored tags are configured.
    pub fn is_empty(&self) -> bool {
        self.require_all.is_empty() && self.ignore_any.is_empty()
    }

    /// Tests a gameplay-tag container against these requirements.
    pub fn passes(&self, tags: Option<&GameplayTagContainer>) -> bool {
        if self.is_empty() {
            return true;
        }

        let Some(tags) = tags else {
            return false;
        };

        tags.has_all_bits(&self.require_all_bits) && !tags.has_any_bits(&self.ignore_any_bits)
    }

    /// Tests a tag slice after expanding inherited tags.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] if a tag is not registered
    /// in `tag_manager`.
    pub fn passes_tag_slice(
        &self,
        tags: &[GameplayTag],
        tag_manager: &GameplayTagManager,
    ) -> Result<bool, GameplayTagError> {
        if self.is_empty() {
            return Ok(true);
        }

        let tag_bits = tag_bits_from_tags_with_manager(tags, tag_manager)?;
        Ok(self.passes_tag_bits(&tag_bits))
    }

    /// Tests a precomputed tag bitset against these requirements.
    pub fn passes_tag_bits(&self, tag_bits: &GameplayTagBits) -> bool {
        if self.is_empty() {
            return true;
        }

        let has_required = tag_bits
            .iter()
            .zip(self.require_all_bits.iter())
            .all(|(owned, required)| (owned & required) == *required);
        let has_blocked = tag_bits
            .iter()
            .zip(self.ignore_any_bits.iter())
            .any(|(owned, blocked)| (owned & blocked) != 0);

        has_required && !has_blocked
    }

    /// Returns the tags that must all be present.
    pub fn get_required_tags(&self) -> &[GameplayTag] {
        &self.require_all
    }

    /// Returns the tags of which none may be present.
    pub fn get_ignored_tags(&self) -> &[GameplayTag] {
        &self.ignore_any
    }
}
