//! Semantic validation and startup compilation of generated table rows.

use super::catalog::{AbilityId, CompiledAbility, EffectId, GameplayCatalog};
use super::generated::{Tables, gas as data};

mod build;
mod error;
mod magnitude;
mod validation;

pub use build::compile_catalog;
pub use error::ConfigError;
pub use validation::validate_tables;

pub(crate) use magnitude::evaluate_linear;
