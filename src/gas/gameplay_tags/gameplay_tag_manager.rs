use super::{GameplayTag, GameplayTagBits, add_bit_with_tag};
use crate::settings::GameplayAbilitySystemSettings;
use crate::unique_names::{UniqueName, UniqueNameError};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use std::error::Error;
use std::fmt;
pub const MAX_TAG_COUNTS: usize = GameplayAbilitySystemSettings::GAMEPLAY_TAG_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameplayTagError {
    UniqueName(UniqueNameError),
    CapacityExceeded { max: usize },
    InvalidTagIndex { index: usize },
}

impl fmt::Display for GameplayTagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameplayTagError::UniqueName(err) => {
                write!(f, "gameplay tag registration failed: {err}")
            }
            GameplayTagError::CapacityExceeded { max } => {
                write!(f, "gameplay tag capacity exceeded; max tags: {max}")
            }
            GameplayTagError::InvalidTagIndex { index } => {
                write!(f, "invalid gameplay tag index: {index}")
            }
        }
    }
}

impl Error for GameplayTagError {}

impl From<UniqueNameError> for GameplayTagError {
    fn from(value: UniqueNameError) -> Self {
        Self::UniqueName(value)
    }
}

#[derive(Resource)]
pub struct GameplayTagManager {
    tag_name_to_index: HashMap<UniqueName, u16>,
    tag_parent_index: Vec<Option<u16>>,
    tag_children: Vec<Vec<u16>>,
    tag_inherited_bits: Vec<GameplayTagBits>,
    next_tag_index: u16,
}

impl Default for GameplayTagManager {
    fn default() -> Self {
        Self {
            tag_name_to_index: HashMap::new(),
            tag_parent_index: Vec::new(),
            tag_children: Vec::new(),
            tag_inherited_bits: Vec::new(),
            next_tag_index: 0,
        }
    }
}

impl GameplayTagManager {
    pub fn get_tag(&self, unique_name: UniqueName) -> Option<GameplayTag> {
        self.tag_name_to_index
            .get(&unique_name)
            .map(|&index| GameplayTag::new(index))
    }

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

        // Create inherited bits: Start with parent's bits or new empty bits
        let mut inherited_bits = parent_tag_index
            .and_then(|p_index| self.tag_inherited_bits.get(p_index as usize).cloned())
            .unwrap_or_default();

        // Set the current tag's own bit in the inherited bits
        let self_tag = GameplayTag::new(new_index);
        add_bit_with_tag(&mut inherited_bits, &self_tag).ok_or(
            GameplayTagError::InvalidTagIndex {
                index: self_tag.get_bit_index_usize(),
            },
        )?;

        // Update the Manager data structures
        if new_index as usize == self.tag_parent_index.len() {
            self.tag_parent_index.push(parent_tag_index);
            self.tag_inherited_bits.push(inherited_bits);
            self.tag_children.push(Vec::new());
        }
        self.tag_name_to_index.insert(unique_name, new_index);
        if let Some(p_index) = parent_tag_index
            && (p_index as usize) < self.tag_children.len()
        {
            self.tag_children[p_index as usize].push(new_index);
        }
        self.next_tag_index += 1;

        Ok(self_tag)
    }

    pub fn get_inherited_bits(&self, tag: &GameplayTag) -> Option<&GameplayTagBits> {
        self.tag_inherited_bits.get(tag.get_bit_index_usize())
    }
    pub fn check_has_active_descendants(&self, tag_index: usize, ref_counts: &[u16]) -> bool {
        let mut stack: Vec<u16> = Vec::new();
        if tag_index < self.tag_children.len() {
            stack.extend(self.tag_children[tag_index].iter().copied());
        } else {
            return false;
        }

        while let Some(current_index) = stack.pop() {
            let index_usize = current_index as usize;
            if index_usize < ref_counts.len() && ref_counts[index_usize] > 0 {
                return true;
            }

            if (index_usize) < self.tag_children.len()
                && let Some(children) = self.tag_children.get(index_usize)
            {
                stack.extend(children.iter().copied());
            }
        }
        false
    }
}
