use super::{GameplayTag, GameplayTagBits, GameplayTagError, MAX_TAG_COUNTS, add_bit_with_tag};
use crate::unique_names::{UniqueName, UniqueNamePool};
use bevy::ecs::system::SystemParam;
use bevy::platform::collections::HashMap;
use bevy::prelude::{ResMut, Resource};

/// Global registry for hierarchical gameplay tags and inherited bitsets.
#[derive(Resource, Default)]
pub struct GameplayTagManager {
    tag_name_to_index: HashMap<UniqueName, u16>,
    tag_parent_index: Vec<Option<u16>>,
    tag_children: Vec<Vec<u16>>,
    tag_inherited_bits: Vec<GameplayTagBits>,
    next_tag_index: u16,
}

impl GameplayTagManager {
    /// Returns the tag registered for `unique_name`, if present.
    pub fn get_tag(&self, unique_name: UniqueName) -> Option<GameplayTag> {
        self.tag_name_to_index
            .get(&unique_name)
            .map(|&index| GameplayTag::new(index))
    }

    /// Registers a tag and its optional direct parent.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::CapacityExceeded`] when the configured tag
    /// capacity is full, or [`GameplayTagError::InvalidTagIndex`] when the
    /// supplied parent index is unknown.
    pub fn register_tag_internal(
        &mut self,
        unique_name: UniqueName,
        parent_tag_index: Option<u16>,
    ) -> Result<GameplayTag, GameplayTagError> {
        if let Some(&index) = self.tag_name_to_index.get(&unique_name) {
            return Ok(GameplayTag::new(index));
        }

        let new_index = self.next_tag_index;
        if new_index as usize >= MAX_TAG_COUNTS {
            return Err(GameplayTagError::CapacityExceeded {
                max: MAX_TAG_COUNTS,
            });
        }

        let mut inherited_bits = match parent_tag_index {
            Some(parent_index) => self
                .tag_inherited_bits
                .get(parent_index as usize)
                .copied()
                .ok_or(GameplayTagError::InvalidTagIndex {
                    index: parent_index as usize,
                })?,
            None => GameplayTagBits::default(),
        };

        let tag = GameplayTag::new(new_index);
        add_bit_with_tag(&mut inherited_bits, &tag)?;

        self.tag_parent_index.push(parent_tag_index);
        self.tag_inherited_bits.push(inherited_bits);
        self.tag_children.push(Vec::new());
        self.tag_name_to_index.insert(unique_name, new_index);
        if let Some(parent_index) = parent_tag_index {
            self.tag_children[parent_index as usize].push(new_index);
        }
        self.next_tag_index += 1;

        Ok(tag)
    }

    /// Returns the cached bitset containing `tag` and all its parents.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] if `tag` is not registered
    /// in this manager.
    pub fn get_inherited_bits(
        &self,
        tag: &GameplayTag,
    ) -> Result<&GameplayTagBits, GameplayTagError> {
        self.tag_inherited_bits
            .get(tag.get_bit_index_usize())
            .ok_or(GameplayTagError::InvalidTagIndex {
                index: tag.get_bit_index_usize(),
            })
    }

    /// Returns whether any registered descendant has a nonzero reference count.
    pub fn check_has_active_descendants(&self, tag_index: usize, ref_counts: &[u16]) -> bool {
        let Some(children) = self.tag_children.get(tag_index) else {
            return false;
        };
        let mut stack = children.clone();

        while let Some(current_index) = stack.pop() {
            let index = current_index as usize;
            if ref_counts.get(index).is_some_and(|count| *count > 0) {
                return true;
            }
            if let Some(children) = self.tag_children.get(index) {
                stack.extend(children.iter().copied());
            }
        }
        false
    }
}

/// Builds a bitset containing the supplied tags and all their parent tags.
///
/// # Errors
///
/// Returns [`GameplayTagError::InvalidTagIndex`] if a tag is not registered in
/// the supplied manager.
pub fn tag_bits_from_tags_with_manager(
    tags: &[GameplayTag],
    manager: &GameplayTagManager,
) -> Result<GameplayTagBits, GameplayTagError> {
    let mut result = GameplayTagBits::default();
    for tag in tags {
        let inherited_bits = manager.get_inherited_bits(tag)?;
        for (destination, source) in result.iter_mut().zip(inherited_bits.iter()) {
            *destination |= *source;
        }
    }
    Ok(result)
}

/// System parameter used to request or register hierarchical tags by name.
#[derive(SystemParam)]
pub struct GameplayTagRegister<'w> {
    unique_name_pool: ResMut<'w, UniqueNamePool>,
    gameplay_tag_manager: ResMut<'w, GameplayTagManager>,
}

impl GameplayTagRegister<'_> {
    /// Returns an existing tag or recursively registers it and its parents.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError`] when name interning, parent registration, or
    /// tag-capacity validation fails.
    pub fn request_or_register_tag(
        &mut self,
        full_tag_name: &str,
    ) -> Result<GameplayTag, GameplayTagError> {
        let unique_name = self.unique_name_pool.new_name(full_tag_name)?;

        if let Some(tag) = self.gameplay_tag_manager.get_tag(unique_name) {
            return Ok(tag);
        }

        let parent_tag_index = full_tag_name
            .rsplit_once('.')
            .map(|(parent_name, _)| {
                self.request_or_register_tag(parent_name)
                    .map(|parent_tag| parent_tag.get_bit_index_u16())
            })
            .transpose()?;

        self.gameplay_tag_manager
            .register_tag_internal(unique_name, parent_tag_index)
    }
}
