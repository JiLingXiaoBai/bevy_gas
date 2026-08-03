use super::{GameplayTagError, GameplayTagManager};
use crate::unique_names::UniqueNamePool;
use bevy::ecs::system::SystemParam;
use bevy::prelude::ResMut;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameplayTag(u16);

impl GameplayTag {
    pub(crate) fn new(tag_bit_index: u16) -> Self {
        Self(tag_bit_index)
    }
    pub fn get_bit_index_u16(&self) -> u16 {
        self.0
    }

    pub fn get_bit_index_usize(&self) -> usize {
        self.0 as usize
    }
}

#[derive(SystemParam)]
pub struct GameplayTagRegister<'w> {
    unique_name_pool: ResMut<'w, UniqueNamePool>,
    gameplay_tag_manager: ResMut<'w, GameplayTagManager>,
}

impl<'w> GameplayTagRegister<'w> {
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
            // Found a parent string (e.g., "Ability.Fireball" -> "Ability")
            .map(|(parent_name, _)| {
                self.request_or_register_tag(parent_name)
                    .map(|parent_tag| parent_tag.0)
            })
            .transpose()?;

        self.gameplay_tag_manager
            .register_tag_internal(unique_name, parent_tag_index)
    }
}
