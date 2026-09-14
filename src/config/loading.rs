//! Binary table loading and human-readable skill inspection.

use super::compiler::ConfigError;
use super::decoding::ByteBuf;
use super::generated::{LubanError, Tables};
use super::read_package;

#[cfg(feature = "luban-config")]
use super::{
    AbilityId,
    compiler::{evaluate_linear, validate_tables},
    generated::gas::ActionKind,
};

mod files;
#[cfg(feature = "luban-config")]
mod inspection;

pub use files::load_tables;
#[cfg(feature = "luban-config")]
pub use inspection::{describe_ability, describe_ability_at_level};
