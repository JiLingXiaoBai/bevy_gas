//! Attribute identifiers, storage, aggregation, snapshots, and recalculation.
//!
//! [`AttributeSet`] owns per-entity values while aggregators retain active
//! duration modifiers. Dirty hot/cold regions are recalculated late in the
//! fixed-tick pipeline.

mod aggregation;
mod attribute_set;
mod registry;
mod snapshot;

pub use aggregation::{Aggregator, default_executor};
pub use attribute_set::{
    ATTRIBUTE_SET_SIZE, AttributePostExecute, AttributeSet, AttributeSetError,
    COLD_ATTRIBUTE_SET_SIZE, HOT_ATTRIBUTE_SET_SIZE, recalculate_attribute_sets_system,
};
pub use registry::{
    AttributeId, AttributeIdError, AttributeIdManager, AttributeIdRegister, AttributeLocation,
    AttributeRegion,
};
pub use snapshot::{AttributeSetSnapshot, AttributeSnapshot};
