//! Deterministic target acquisition through the synchronized dynamic AABB index.

use super::{
    AbilityTargetData, AbilityTargetHit, Targetable, TargetingDefinition, TargetingError,
    TargetingInput, TargetingOperation, TargetingSortOrder, TargetingSpatialIndex,
};
use crate::attributes::AttributeSet;
use crate::gameplay_position::LogicalPosition;
use crate::gameplay_tags::GameplayTagContainer;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// Read-only ECS query used to validate spatial candidates and apply gameplay filters.
pub type TargetingCandidateQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static LogicalPosition,
        Option<&'static GameplayTagContainer>,
        Option<&'static AttributeSet>,
        Option<&'static Targetable>,
    ),
>;

/// Shared read-only parameters for synchronous and queued target acquisition.
///
/// Range selections use the spatial index exclusively. Schedule calls after
/// `sync_targeting_spatial_index_system` and before further changes to target positions
/// or membership. The runtime plugin establishes this order for queued requests.
#[derive(SystemParam)]
pub struct TargetingSystemParams<'w, 's> {
    index: Res<'w, TargetingSpatialIndex>,
    query: TargetingCandidateQuery<'w, 's>,
}

#[derive(Clone, Copy)]
struct Candidate {
    hit: AbilityTargetHit,
    distance_squared: f64,
}

/// Executes a validated targeting definition against the synchronized spatial index and ECS.
///
/// `source` identifies the requester, `input` captures logical world-space input, and `definition`
/// supplies ordered selection and refinement operations. Build input origins from logical
/// positions, never interpolated visual transforms. Returns non-empty, ordered hits.
/// Tree traversal order never affects results: selections begin in entity-bit order,
/// and distance sorts use entity bits as their tie breaker.
///
/// # Errors
///
/// Returns [`TargetingError`] for invalid input, missing entities or components, an
/// uninitialized range index, an invalid explicit target, or an empty final candidate set.
/// Non-finite positions are excluded from range selections and rejected for direct targets.
pub fn acquire_targets(
    source: Entity,
    input: TargetingInput,
    definition: &TargetingDefinition,
    params: &TargetingSystemParams,
) -> Result<AbilityTargetData, TargetingError> {
    let origin = input.get_origin();
    if !origin.is_finite() {
        return Err(TargetingError::InvalidOrigin);
    }

    let Some((selection, refinements)) = definition.get_operations().split_first() else {
        return Err(TargetingError::InvalidDefinition);
    };
    let mut candidates = select_candidates(selection, source, input, origin, params)?;
    for operation in refinements {
        refine_candidates(operation, source, &params.query, &mut candidates)?;
    }

    if candidates.is_empty() {
        return Err(TargetingError::NoTargetsFound);
    }

    Ok(AbilityTargetData::new(
        origin,
        candidates
            .into_iter()
            .map(|candidate| candidate.hit)
            .collect(),
    ))
}

fn select_candidates(
    selection: &TargetingOperation,
    source: Entity,
    input: TargetingInput,
    origin: Vec3,
    params: &TargetingSystemParams,
) -> Result<Vec<Candidate>, TargetingError> {
    match selection {
        TargetingOperation::SelectSelf => {
            let Ok((_, position, _, _, _)) = params.query.get(source) else {
                return Err(TargetingError::MissingSource { source });
            };
            Ok(vec![candidate(source, position.0, origin)?])
        }
        TargetingOperation::SelectExplicitEntity => {
            let Some(target) = input.get_explicit_target() else {
                return Err(TargetingError::MissingExplicitTarget);
            };
            let Ok((_, position, _, _, targetable)) = params.query.get(target) else {
                return Err(TargetingError::TargetNotFound { target });
            };
            if targetable.is_none() {
                return Err(TargetingError::TargetNotTargetable { target });
            }
            Ok(vec![candidate(target, position.0, origin)?])
        }
        TargetingOperation::SelectSphere { radius } => {
            select_spatial_candidates(*radius, origin, params)
        }
        TargetingOperation::SelectCone {
            radius,
            half_angle_radians,
        } => select_cone_candidates(*radius, *half_angle_radians, input, origin, params),
        _ => Err(TargetingError::InvalidDefinition),
    }
}

fn select_spatial_candidates(
    radius: f32,
    origin: Vec3,
    params: &TargetingSystemParams,
) -> Result<Vec<Candidate>, TargetingError> {
    if !params.index.is_ready() {
        return Err(TargetingError::SpatialIndexNotReady);
    }
    let mut entities = Vec::new();
    params.index.query(origin, radius, &mut entities);
    let radius_squared = f64::from(radius).powi(2);
    let mut candidates = entities
        .into_iter()
        .filter_map(|entity| {
            let (_, position, _, _, targetable) = params.query.get(entity).ok()?;
            targetable?;
            let candidate = candidate(entity, position.0, origin).ok()?;
            (candidate.distance_squared <= radius_squared).then_some(candidate)
        })
        .collect::<Vec<_>>();
    sort_by_entity(&mut candidates);
    Ok(candidates)
}

fn select_cone_candidates(
    radius: f32,
    half_angle_radians: f32,
    input: TargetingInput,
    origin: Vec3,
    params: &TargetingSystemParams,
) -> Result<Vec<Candidate>, TargetingError> {
    let direction = input.get_direction().as_dvec3();
    if !direction.is_finite() || direction.length_squared() <= f64::from(f32::EPSILON) {
        return Err(TargetingError::InvalidDirection);
    }
    let direction = direction.normalize();
    let minimum_dot = f64::from(half_angle_radians.cos());
    let mut candidates = select_spatial_candidates(radius, origin, params)?;
    candidates.retain(|candidate| {
        if candidate.distance_squared <= f64::from(f32::EPSILON) {
            return true;
        }
        let offset = candidate.hit.get_position().as_dvec3() - origin.as_dvec3();
        offset.normalize().dot(direction) >= minimum_dot
    });
    Ok(candidates)
}

fn refine_candidates(
    operation: &TargetingOperation,
    source: Entity,
    query: &TargetingCandidateQuery,
    candidates: &mut Vec<Candidate>,
) -> Result<(), TargetingError> {
    match operation {
        TargetingOperation::FilterSource => {
            candidates.retain(|candidate| candidate.hit.get_entity() != source);
        }
        TargetingOperation::FilterTags { requirements } => {
            candidates.retain(|candidate| {
                query
                    .get(candidate.hit.get_entity())
                    .ok()
                    .is_some_and(|(_, _, tags, _, _)| requirements.passes(tags))
            });
        }
        TargetingOperation::RequireAttributeSet => {
            candidates.retain(|candidate| {
                query
                    .get(candidate.hit.get_entity())
                    .ok()
                    .is_some_and(|(_, _, _, attributes, _)| attributes.is_some())
            });
        }
        TargetingOperation::FilterDistance { max_distance } => {
            let max_distance_squared = f64::from(*max_distance).powi(2);
            candidates.retain(|candidate| candidate.distance_squared <= max_distance_squared);
        }
        TargetingOperation::SortByDistance { order } => sort_by_distance(candidates, *order),
        TargetingOperation::Limit { count } => candidates.truncate(*count),
        _ => return Err(TargetingError::InvalidDefinition),
    }
    Ok(())
}

fn sort_by_distance(candidates: &mut [Candidate], order: TargetingSortOrder) {
    candidates.sort_by(|left, right| {
        let distance_order = left.distance_squared.total_cmp(&right.distance_squared);
        let order = match order {
            TargetingSortOrder::Ascending => distance_order,
            TargetingSortOrder::Descending => distance_order.reverse(),
        };
        order.then_with(|| {
            left.hit
                .get_entity()
                .to_bits()
                .cmp(&right.hit.get_entity().to_bits())
        })
    });
}

fn candidate(entity: Entity, position: Vec3, origin: Vec3) -> Result<Candidate, TargetingError> {
    if !position.is_finite() {
        return Err(TargetingError::InvalidTargetPosition { target: entity });
    }
    // Widen before subtraction and squaring so finite f32 coordinates cannot overflow.
    Ok(Candidate {
        hit: AbilityTargetHit::new(entity, position, None),
        distance_squared: position.as_dvec3().distance_squared(origin.as_dvec3()),
    })
}

fn sort_by_entity(candidates: &mut [Candidate]) {
    candidates.sort_by_key(|candidate| candidate.hit.get_entity().to_bits());
}
