//! Startup compilation and optional authoring validation of generated table rows.

use super::catalog::{AbilityId, CompiledAbility, EffectId, GameplayCatalog};
use super::generated::{Tables, gas as data};
use super::{ConfigError, ConfigErrorKind, ConfigLocation};

mod abilities;
mod build;
mod effects;
mod magnitude;
mod numeric;
mod preparation;
mod registration;
mod targeting;
#[cfg(feature = "config-validation")]
mod validation;

pub use build::compile_catalog;
#[cfg(feature = "config-validation")]
pub use validation::validate_tables;

pub(crate) use preparation::{
    PreparedActionKind, PreparedMagnitude, PreparedTables, PreparedTargetScope,
};
#[cfg(feature = "config-validation")]
pub(crate) use validation::validate_prepared;
