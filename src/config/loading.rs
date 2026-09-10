//! Binary table loading and human-readable skill inspection.

use super::compiler::{ConfigError, evaluate_linear, validate_tables};
use super::decoding::ByteBuf;
use super::generated::{LubanError, Tables, gas::ActionKind};
use super::{AbilityId, read_package};

mod files;
mod inspection;

pub use files::load_tables;
pub use inspection::{describe_ability, describe_ability_at_level};
