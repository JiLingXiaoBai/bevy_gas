//! Configuration errors shared by loading, compilation, tooling, and runtime grants.

mod definition;

pub use definition::{ConfigError, ConfigErrorKind, ConfigLocation};
