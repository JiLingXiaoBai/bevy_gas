//! Interned unique names shared by gameplay registries.

mod error;
mod unique_name;

pub use error::UniqueNameError;
pub use unique_name::{UniqueName, UniqueNamePool};
