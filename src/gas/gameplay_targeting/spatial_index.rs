//! Dynamic spatial indexing for point-based gameplay target acquisition.

mod index;
mod tree;

pub use index::{TargetingSpatialIndex, sync_targeting_spatial_index_system};
pub(crate) use index::{remove_discarded_target_position, remove_discarded_targetable};
