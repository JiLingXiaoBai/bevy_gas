//! Plugin composition and ordered fixed-tick scheduling for the GAS runtime.

use crate::ability_system::{PendingActiveGameplayAbilities, cleanup_finished_abilities_system};
use crate::attributes::{AttributeIdManager, recalculate_attribute_sets_system};
use crate::gameplay_abilities::tick_ability_tasks_system;
use crate::gameplay_effects::{
    ActiveEffectRequirementSync, tick_effect_duration_system, tick_effect_period_system,
    update_active_effect_tag_requirements_system,
};
use crate::gameplay_execution::{
    GameplayExecutionQueue, gameplay_execution_queue_has_work,
    process_gameplay_execution_queue_system,
};
use crate::gameplay_tags::GameplayTagManager;
use crate::gameplay_targeting::{
    TargetingRequestQueue, process_targeting_request_queue_system, targeting_request_queue_has_work,
};
use crate::randoms::Random;
use crate::unique_names::UniqueNamePool;
use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

/// Installs the resource used to register and resolve gameplay tags.
pub struct GameplayTagPlugin;

impl Plugin for GameplayTagPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameplayTagManager>();
    }
}

/// Installs the shared unique-name pool used by registered gameplay identifiers.
pub struct UniqueNamePlugin;

impl Plugin for UniqueNamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UniqueNamePool>();
    }
}

/// Installs the deterministic random resource used by gameplay evaluation.
pub struct RandomPlugin;

impl Plugin for RandomPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Random>();
    }
}

/// Installs the fixed-tick GAS runtime resources, phases, and systems.
pub struct GameplayAbilitySystemRuntimePlugin;

/// Ordered phases of the fixed-tick GAS runtime pipeline.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum GameplayAbilitySystemSet {
    /// Advances effect duration, requirements, and period state.
    EffectTicks,
    /// Runs built-in ability tasks that can produce gameplay requests.
    AbilityTasks,
    /// Public phase for gameplay request-producing systems.
    RequestProducers,
    /// Runs targeting work that may produce gameplay requests.
    Targeting,
    /// Converges externally changed tags before gameplay requests are consumed.
    PreGameplayConvergence,
    /// Drains the deterministic gameplay execution FIFO.
    GameplayResolve,
    /// Converges tag requirements after gameplay execution.
    UpdateEffectTagRequirements,
    /// Removes ending and cancelled ability instances.
    Cleanup,
    /// Recalculates dirty attributes after all gameplay mutations.
    RecalculateAttributes,
}

impl Plugin for GameplayAbilitySystemRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AttributeIdManager>()
            .init_resource::<GameplayExecutionQueue>()
            .init_resource::<ActiveEffectRequirementSync>()
            .init_resource::<PendingActiveGameplayAbilities>()
            .init_resource::<TargetingRequestQueue>()
            .configure_sets(
                FixedUpdate,
                (
                    GameplayAbilitySystemSet::EffectTicks
                        .before(GameplayAbilitySystemSet::AbilityTasks),
                    GameplayAbilitySystemSet::AbilityTasks
                        .before(GameplayAbilitySystemSet::RequestProducers),
                    GameplayAbilitySystemSet::RequestProducers
                        .before(GameplayAbilitySystemSet::Targeting),
                    GameplayAbilitySystemSet::Targeting
                        .before(GameplayAbilitySystemSet::PreGameplayConvergence),
                    GameplayAbilitySystemSet::PreGameplayConvergence
                        .before(GameplayAbilitySystemSet::GameplayResolve),
                    GameplayAbilitySystemSet::GameplayResolve
                        .before(GameplayAbilitySystemSet::UpdateEffectTagRequirements),
                    GameplayAbilitySystemSet::UpdateEffectTagRequirements
                        .before(GameplayAbilitySystemSet::Cleanup),
                    GameplayAbilitySystemSet::Cleanup
                        .before(GameplayAbilitySystemSet::RecalculateAttributes),
                ),
            )
            .add_systems(
                FixedUpdate,
                (
                    tick_effect_duration_system,
                    update_active_effect_tag_requirements_system,
                    tick_effect_period_system,
                    update_active_effect_tag_requirements_system,
                )
                    .chain()
                    .in_set(GameplayAbilitySystemSet::EffectTicks),
            )
            .add_systems(
                FixedUpdate,
                tick_ability_tasks_system.in_set(GameplayAbilitySystemSet::AbilityTasks),
            )
            .add_systems(
                FixedUpdate,
                process_targeting_request_queue_system
                    .run_if(targeting_request_queue_has_work)
                    .in_set(GameplayAbilitySystemSet::Targeting),
            )
            .add_systems(
                FixedUpdate,
                update_active_effect_tag_requirements_system
                    .in_set(GameplayAbilitySystemSet::PreGameplayConvergence),
            )
            .add_systems(
                FixedUpdate,
                process_gameplay_execution_queue_system
                    .run_if(gameplay_execution_queue_has_work)
                    .in_set(GameplayAbilitySystemSet::GameplayResolve),
            )
            .add_systems(
                FixedUpdate,
                update_active_effect_tag_requirements_system
                    .in_set(GameplayAbilitySystemSet::UpdateEffectTagRequirements),
            )
            .add_systems(
                FixedUpdate,
                cleanup_finished_abilities_system.in_set(GameplayAbilitySystemSet::Cleanup),
            )
            .add_systems(
                FixedUpdate,
                recalculate_attribute_sets_system
                    .in_set(GameplayAbilitySystemSet::RecalculateAttributes),
            );
    }
}

/// Plugin group containing all resources and runtime systems required by GAS.
pub struct GameplayAbilitySystemPlugin;

impl PluginGroup for GameplayAbilitySystemPlugin {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(UniqueNamePlugin)
            .add(GameplayTagPlugin)
            .add(RandomPlugin)
            .add(GameplayAbilitySystemRuntimePlugin)
    }
}
