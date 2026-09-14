//! Target selection and refinement pipeline construction.

use super::numeric::square_is_finite;
use super::registration::resolve_tags;
use super::{ConfigError, data};
use crate::{
    GameplayTag, TagRequirements, TargetingDefinition, TargetingOperation, TargetingSortOrder,
};
use std::collections::BTreeMap;

pub(super) fn compile_targeting(
    row: &data::Targeting,
    tags: &BTreeMap<String, GameplayTag>,
) -> Result<TargetingDefinition, ConfigError> {
    let context = format!("Targeting[{}]", row.id);
    let used_radius = if matches!(
        row.selection,
        data::SelectionKind::Sphere | data::SelectionKind::Cone
    ) {
        row.radius
    } else {
        None
    };
    for (name, distance) in [("radius", used_radius), ("max_distance", row.max_distance)] {
        if distance.is_some_and(|value| !square_is_finite(value)) {
            return Err(ConfigError::new(
                format!("{context}.{name}"),
                "distance must have a finite square",
            ));
        }
    }
    let selection = match row.selection {
        data::SelectionKind::SelfTarget => TargetingOperation::SelectSelf,
        data::SelectionKind::ExplicitEntity => TargetingOperation::SelectExplicitEntity,
        data::SelectionKind::Sphere => TargetingOperation::SelectSphere {
            radius: row
                .radius
                .ok_or_else(|| ConfigError::new(&context, "missing radius"))?,
        },
        data::SelectionKind::Cone => TargetingOperation::SelectCone {
            radius: row
                .radius
                .ok_or_else(|| ConfigError::new(&context, "missing radius"))?,
            half_angle_radians: row
                .half_angle_radians
                .ok_or_else(|| ConfigError::new(&context, "missing cone angle"))?,
        },
    };
    let mut operations = vec![selection];
    if row.exclude_source {
        operations.push(TargetingOperation::FilterSource);
    }
    if !row.required_tags.is_empty() || !row.blocked_tags.is_empty() {
        let requirements = TagRequirements::new(
            resolve_tags(&row.required_tags, tags)?,
            resolve_tags(&row.blocked_tags, tags)?,
        )
        .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        operations.push(TargetingOperation::FilterTags { requirements });
    }
    if row.require_attributes {
        operations.push(TargetingOperation::RequireAttributeSet);
    }
    if let Some(max_distance) = row.max_distance {
        operations.push(TargetingOperation::FilterDistance { max_distance });
    }
    match row.sort {
        data::SortOrder::None => {}
        data::SortOrder::Nearest => operations.push(TargetingOperation::SortByDistance {
            order: TargetingSortOrder::Ascending,
        }),
        data::SortOrder::Farthest => operations.push(TargetingOperation::SortByDistance {
            order: TargetingSortOrder::Descending,
        }),
    }
    operations.push(TargetingOperation::Limit {
        count: usize::try_from(row.limit)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?,
    });
    TargetingDefinition::new(operations)
        .map_err(|error| ConfigError::new(context, error.to_string()))
}
