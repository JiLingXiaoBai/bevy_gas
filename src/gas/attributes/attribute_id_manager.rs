use super::ATTRIBUTE_SET_SIZE;
use crate::settings::GameplayAbilitySystemSettings;
use crate::{UniqueName, UniqueNameError, UniqueNamePool};
use bevy::ecs::system::SystemParam;
use bevy::platform::collections::HashMap;
use bevy::prelude::{ResMut, Resource};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttributeId(u16);

impl AttributeId {
    pub(crate) fn new(index: u16) -> Self {
        Self(index)
    }

    /// Returns the ordinary global index assigned to this attribute.
    pub fn to_index(self) -> usize {
        self.0 as usize
    }
}

/// The storage region assigned to an attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeRegion {
    /// Frequently accessed attributes stored in the compact hot region.
    Hot,
    /// Less frequently accessed attributes stored in the cold region.
    Cold,
}

/// The physical storage location assigned to an attribute ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttributeLocation {
    region: AttributeRegion,
    slot: usize,
}

impl AttributeLocation {
    pub(crate) const fn new(region: AttributeRegion, slot: usize) -> Self {
        Self { region, slot }
    }

    /// Returns the storage region containing the attribute.
    pub const fn region(self) -> AttributeRegion {
        self.region
    }

    /// Returns the attribute's index within its storage region.
    pub const fn slot(self) -> usize {
        self.slot
    }

    pub(crate) const fn sort_key(self) -> (u8, usize) {
        let region = match self.region {
            AttributeRegion::Hot => 0,
            AttributeRegion::Cold => 1,
        };
        (region, self.slot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeIdError {
    /// The attribute name could not be interned.
    UniqueName(UniqueNameError),
    /// The combined hot and cold attribute capacity was exceeded.
    CapacityExceeded { max: usize },
    /// One storage region reached its configured capacity.
    RegionCapacityExceeded { region: AttributeRegion, max: usize },
    /// An existing attribute was requested with a different region.
    RegionMismatch {
        existing: AttributeRegion,
        requested: AttributeRegion,
    },
    /// Internal registration data was inconsistent for an existing ID.
    MissingLocation { id: AttributeId },
}

impl fmt::Display for AttributeIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttributeIdError::UniqueName(err) => {
                write!(f, "attribute ID registration failed: {err}")
            }
            AttributeIdError::CapacityExceeded { max } => {
                write!(f, "attribute id capacity exceeded; max attributes: {max}")
            }
            AttributeIdError::RegionCapacityExceeded { region, max } => {
                write!(
                    f,
                    "{region:?} attribute capacity exceeded; max attributes in region: {max}"
                )
            }
            AttributeIdError::RegionMismatch {
                existing,
                requested,
            } => write!(
                f,
                "attribute was already registered as {existing:?}, but {requested:?} was requested"
            ),
            AttributeIdError::MissingLocation { id } => write!(
                f,
                "attribute ID {} is missing its storage location",
                id.to_index()
            ),
        }
    }
}

impl Error for AttributeIdError {}

impl From<UniqueNameError> for AttributeIdError {
    fn from(value: UniqueNameError) -> Self {
        Self::UniqueName(value)
    }
}

/// Global manager for attribute IDs and their hot or cold storage locations.
#[derive(Resource, Clone)]
pub struct AttributeIdManager {
    name_to_index: HashMap<UniqueName, u16>,
    next_id_index: u16,
    locations: [Option<AttributeLocation>; GameplayAbilitySystemSettings::ATTRIBUTE_SET_SIZE],
    hot_count: usize,
    cold_count: usize,
}

impl Default for AttributeIdManager {
    fn default() -> Self {
        Self {
            name_to_index: HashMap::new(),
            next_id_index: 0,
            locations: [None; GameplayAbilitySystemSettings::ATTRIBUTE_SET_SIZE],
            hot_count: 0,
            cold_count: 0,
        }
    }
}

impl AttributeIdManager {
    /// Returns the ID registered for `unique_name`, if present.
    pub fn get_attribute_id(&self, unique_name: UniqueName) -> Option<AttributeId> {
        self.name_to_index
            .get(&unique_name)
            .map(|&id| AttributeId::new(id))
    }

    /// Returns the storage location assigned to `id`.
    ///
    /// # Errors
    ///
    /// Returns [`AttributeIdError::MissingLocation`] if `id` is not registered
    /// in this manager.
    pub fn location(&self, id: AttributeId) -> Result<AttributeLocation, AttributeIdError> {
        self.locations
            .get(id.to_index())
            .copied()
            .flatten()
            .ok_or(AttributeIdError::MissingLocation { id })
    }

    /// Returns the number of registered hot attributes.
    pub const fn hot_count(&self) -> usize {
        self.hot_count
    }

    /// Returns the number of registered cold attributes.
    pub const fn cold_count(&self) -> usize {
        self.cold_count
    }

    /// Registers an attribute ID and assigns its hot or cold storage slot.
    pub fn register_id_internal(
        &mut self,
        unique_name: UniqueName,
        region: AttributeRegion,
    ) -> Result<AttributeId, AttributeIdError> {
        if let Some(&index) = self.name_to_index.get(&unique_name) {
            let id = AttributeId::new(index);
            let existing_location = self.location(id)?;
            if existing_location.region() != region {
                return Err(AttributeIdError::RegionMismatch {
                    existing: existing_location.region(),
                    requested: region,
                });
            }
            return Ok(id);
        }

        let new_index = self.next_id_index;
        if new_index as usize >= ATTRIBUTE_SET_SIZE {
            return Err(AttributeIdError::CapacityExceeded {
                max: ATTRIBUTE_SET_SIZE,
            });
        }

        let Some(slot) = self.next_slot(region) else {
            let max = match region {
                AttributeRegion::Hot => GameplayAbilitySystemSettings::HOT_ATTRIBUTE_SET_SIZE,
                AttributeRegion::Cold => GameplayAbilitySystemSettings::COLD_ATTRIBUTE_SET_SIZE,
            };
            return Err(AttributeIdError::RegionCapacityExceeded { region, max });
        };

        let attribute_id = AttributeId::new(new_index);
        self.name_to_index.insert(unique_name, new_index);
        self.next_id_index += 1;
        self.assign_location(attribute_id, region, slot);
        Ok(attribute_id)
    }

    fn next_slot(&self, region: AttributeRegion) -> Option<usize> {
        match region {
            AttributeRegion::Hot => (self.hot_count
                < GameplayAbilitySystemSettings::HOT_ATTRIBUTE_SET_SIZE)
                .then_some(self.hot_count),
            AttributeRegion::Cold => (self.cold_count
                < GameplayAbilitySystemSettings::COLD_ATTRIBUTE_SET_SIZE)
                .then_some(self.cold_count),
        }
    }

    fn assign_location(&mut self, id: AttributeId, region: AttributeRegion, slot: usize) {
        debug_assert!(self.locations[id.to_index()].is_none());
        self.locations[id.to_index()] = Some(AttributeLocation::new(region, slot));
        match region {
            AttributeRegion::Hot => self.hot_count += 1,
            AttributeRegion::Cold => self.cold_count += 1,
        }
    }
}

#[derive(SystemParam)]
pub struct AttributeIdRegister<'w> {
    unique_name_pool: ResMut<'w, UniqueNamePool>,
    attribute_id_manager: ResMut<'w, AttributeIdManager>,
}

impl<'w> AttributeIdRegister<'w> {
    /// Returns an existing attribute ID or registers it in `region`.
    pub fn request_or_register_attribute_id(
        &mut self,
        attribute_id_name: &str,
        region: AttributeRegion,
    ) -> Result<AttributeId, AttributeIdError> {
        let unique_name = self.unique_name_pool.new_name(attribute_id_name)?;
        self.attribute_id_manager
            .register_id_internal(unique_name, region)
    }
}
