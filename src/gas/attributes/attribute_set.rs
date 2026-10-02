//! Attribute-set state, mutation, and dirty-value recalculation.

mod error;
mod mutation;
mod recalculation;
mod state;

pub use error::AttributeSetError;
pub use recalculation::recalculate_attribute_sets_system;
pub use state::{
    ATTRIBUTE_SET_SIZE, AttributePostExecute, AttributeSet, COLD_ATTRIBUTE_SET_SIZE,
    HOT_ATTRIBUTE_SET_SIZE,
};
