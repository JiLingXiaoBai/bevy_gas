//! Feature-enabled inspection reports for authored ability tables.

use super::compiler::{evaluate_linear, validate_tables};
use super::generated::{Tables, gas::ActionKind};
use super::{AbilityId, ConfigError, ConfigErrorKind, ConfigLocation};

mod report;

pub use report::{describe_ability, describe_ability_at_level};
