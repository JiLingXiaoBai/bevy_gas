//! Feature-enabled inspection reports for authored ability tables.

use super::compiler::{PreparedActionKind, PreparedTables, validate_prepared};
use super::generated::Tables;
use super::{AbilityId, ConfigError, ConfigErrorKind, ConfigLocation};

mod report;

pub use report::{describe_ability, describe_ability_at_level};
