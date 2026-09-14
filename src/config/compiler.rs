//! Startup compilation and optional authoring validation of generated table rows.

use super::ConfigError;
use super::catalog::{AbilityId, CompiledAbility, EffectId, GameplayCatalog};
use super::generated::{Tables, gas as data};

mod abilities;
mod build;
mod effects;
mod magnitude;
mod numeric;
mod registration;
mod targeting;
#[cfg(feature = "luban-config")]
mod validation;

pub use build::compile_catalog;
#[cfg(feature = "luban-config")]
pub use validation::validate_tables;

#[cfg(feature = "luban-config")]
pub(crate) use magnitude::evaluate_linear;
