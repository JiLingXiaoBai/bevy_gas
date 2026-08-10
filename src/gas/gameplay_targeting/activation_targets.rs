use super::AbilityTargetData;
use bevy::prelude::Entity;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// An error returned when constructing validated ability activation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityActivationTargetsError {
    /// Acquired target data did not contain a primary target.
    EmptyTargetData,
}

impl Display for AbilityActivationTargetsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTargetData => {
                formatter.write_str("ability activation target data must not be empty")
            }
        }
    }
}

impl Error for AbilityActivationTargetsError {}

/// Owns the complete, validated target selection for one ability activation.
///
/// A selection is either one explicit entity or non-empty acquired target data. The cached
/// primary target is established by the constructors so downstream execution cannot observe a
/// disagreement between a fallback target and the first acquired target.
#[derive(Debug, Clone, PartialEq)]
pub struct AbilityActivationTargets {
    primary_target: Entity,
    target_data: Option<AbilityTargetData>,
}

impl AbilityActivationTargets {
    /// Creates a selection containing one explicit target entity.
    pub fn single(target: Entity) -> Self {
        Self {
            primary_target: target,
            target_data: None,
        }
    }

    /// Creates a selection from non-empty acquired target data.
    ///
    /// # Errors
    ///
    /// Returns [`AbilityActivationTargetsError::EmptyTargetData`] when `target_data` contains no
    /// hits and therefore cannot provide a primary target.
    pub fn acquired(target_data: AbilityTargetData) -> Result<Self, AbilityActivationTargetsError> {
        let Some(primary_target) = target_data.primary_entity() else {
            return Err(AbilityActivationTargetsError::EmptyTargetData);
        };

        Ok(Self {
            primary_target,
            target_data: Some(target_data),
        })
    }

    /// Returns the primary entity used by single-target execution paths.
    pub fn get_primary_target(&self) -> Entity {
        self.primary_target
    }

    /// Returns acquired target data, or `None` for an explicit single-target selection.
    pub fn get_target_data(&self) -> Option<&AbilityTargetData> {
        self.target_data.as_ref()
    }

    /// Iterates over every target in deterministic execution order.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.target_data
            .as_ref()
            .into_iter()
            .flat_map(AbilityTargetData::entities)
            .chain(self.target_data.is_none().then_some(self.primary_target))
    }
}

impl From<Entity> for AbilityActivationTargets {
    fn from(target: Entity) -> Self {
        Self::single(target)
    }
}

impl TryFrom<AbilityTargetData> for AbilityActivationTargets {
    type Error = AbilityActivationTargetsError;

    fn try_from(target_data: AbilityTargetData) -> Result<Self, Self::Error> {
        Self::acquired(target_data)
    }
}
