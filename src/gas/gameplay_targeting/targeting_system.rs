use super::{
    AbilityTargetData, AbilityTargetHit, Targetable, TargetingDefinition, TargetingInput,
    TargetingOperation, TargetingSortOrder,
};
use crate::attributes::AttributeSet;
use crate::gameplay_tags::GameplayTagContainer;
use bevy::prelude::*;
use std::error::Error;
use std::fmt;

/// Read-only ECS query used to collect and filter targeting candidates.
pub type TargetingCandidateQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static GlobalTransform,
        Option<&'static GameplayTagContainer>,
        Option<&'static AttributeSet>,
        Option<&'static Targetable>,
    ),
>;

/// Describes why a targeting request could not produce target data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetingError {
    InvalidDefinition,
    InvalidOrigin,
    InvalidDirection,
    MissingSource { source: Entity },
    MissingExplicitTarget,
    TargetNotFound { target: Entity },
    TargetNotTargetable { target: Entity },
    NoTargetsFound,
}

impl fmt::Display for TargetingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDefinition => write!(f, "targeting definition is not valid"),
            Self::InvalidOrigin => write!(f, "targeting origin must contain finite coordinates"),
            Self::InvalidDirection => write!(f, "targeting direction must be finite and non-zero"),
            Self::MissingSource { source } => {
                write!(
                    f,
                    "targeting source {source:?} does not have a GlobalTransform"
                )
            }
            Self::MissingExplicitTarget => {
                write!(f, "explicit target selection requires an explicit entity")
            }
            Self::TargetNotFound { target } => {
                write!(
                    f,
                    "target entity {target:?} does not have a GlobalTransform"
                )
            }
            Self::TargetNotTargetable { target } => {
                write!(f, "target entity {target:?} is not marked Targetable")
            }
            Self::NoTargetsFound => write!(f, "targeting request found no valid targets"),
        }
    }
}

impl Error for TargetingError {}

#[derive(Clone, Copy)]
struct Candidate {
    hit: AbilityTargetHit,
    distance_squared: f32,
}

/// Executes a validated targeting definition against the current ECS state.
///
/// Query iteration order never affects the returned order: selections begin in
/// entity-bit order, and distance sorts use entity bits as their tie breaker.
///
/// # Errors
///
/// Returns [`TargetingError`] for invalid spatial input, missing entities or
/// components, an invalid explicit target, or an empty final candidate set.
pub fn acquire_targets(
    source: Entity,
    input: TargetingInput,
    definition: &TargetingDefinition,
    query: &TargetingCandidateQuery,
) -> Result<AbilityTargetData, TargetingError> {
    let origin = input.get_origin();
    if !origin.is_finite() {
        return Err(TargetingError::InvalidOrigin);
    }

    let Some((selection, refinements)) = definition.get_operations().split_first() else {
        return Err(TargetingError::InvalidDefinition);
    };
    let mut candidates = select_candidates(selection, source, input, origin, query)?;
    for operation in refinements {
        refine_candidates(operation, source, query, &mut candidates)?;
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
    query: &TargetingCandidateQuery,
) -> Result<Vec<Candidate>, TargetingError> {
    match selection {
        TargetingOperation::SelectSelf => {
            let Ok((_, transform, _, _, _)) = query.get(source) else {
                return Err(TargetingError::MissingSource { source });
            };
            Ok(vec![candidate(source, transform.translation(), origin)])
        }
        TargetingOperation::SelectExplicitEntity => {
            let Some(target) = input.get_explicit_target() else {
                return Err(TargetingError::MissingExplicitTarget);
            };
            let Ok((_, transform, _, _, targetable)) = query.get(target) else {
                return Err(TargetingError::TargetNotFound { target });
            };
            if targetable.is_none() {
                return Err(TargetingError::TargetNotTargetable { target });
            }
            Ok(vec![candidate(target, transform.translation(), origin)])
        }
        TargetingOperation::SelectSphere { radius } => {
            let radius_squared = radius * radius;
            let mut candidates = query
                .iter()
                .filter_map(|(entity, transform, _, _, targetable)| {
                    targetable?;
                    let candidate = candidate(entity, transform.translation(), origin);
                    (candidate.distance_squared <= radius_squared).then_some(candidate)
                })
                .collect::<Vec<_>>();
            sort_by_entity(&mut candidates);
            Ok(candidates)
        }
        TargetingOperation::SelectCone {
            radius,
            half_angle_radians,
        } => select_cone_candidates(*radius, *half_angle_radians, input, origin, query),
        _ => Err(TargetingError::InvalidDefinition),
    }
}

fn select_cone_candidates(
    radius: f32,
    half_angle_radians: f32,
    input: TargetingInput,
    origin: Vec3,
    query: &TargetingCandidateQuery,
) -> Result<Vec<Candidate>, TargetingError> {
    let direction = input.get_direction();
    if !direction.is_finite() || direction.length_squared() <= f32::EPSILON {
        return Err(TargetingError::InvalidDirection);
    }
    let direction = direction.normalize();
    let radius_squared = radius * radius;
    let minimum_dot = half_angle_radians.cos();
    let mut candidates = query
        .iter()
        .filter_map(|(entity, transform, _, _, targetable)| {
            targetable?;
            let offset = transform.translation() - origin;
            let distance_squared = offset.length_squared();
            if distance_squared > radius_squared
                || (distance_squared > f32::EPSILON
                    && offset.normalize().dot(direction) < minimum_dot)
            {
                return None;
            }
            Some(Candidate {
                hit: AbilityTargetHit::new(entity, transform.translation(), None),
                distance_squared,
            })
        })
        .collect::<Vec<_>>();
    sort_by_entity(&mut candidates);
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
            let max_distance_squared = max_distance * max_distance;
            candidates.retain(|candidate| candidate.distance_squared <= max_distance_squared);
        }
        TargetingOperation::SortByDistance { order } => {
            sort_by_distance(candidates, *order);
        }
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

fn candidate(entity: Entity, position: Vec3, origin: Vec3) -> Candidate {
    Candidate {
        hit: AbilityTargetHit::new(entity, position, None),
        distance_squared: position.distance_squared(origin),
    }
}

fn sort_by_entity(candidates: &mut [Candidate]) {
    candidates.sort_by_key(|candidate| candidate.hit.get_entity().to_bits());
}
