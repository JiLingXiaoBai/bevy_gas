use crate::ability_system::{
    AdditionalCostProvider, PendingActiveGameplayAbilities, cleanup_discarded_ability_system,
    cleanup_discarded_active_ability, cleanup_finished_abilities_system,
};
use crate::attributes::{AttributeIdManager, recalculate_attribute_sets_system};
use crate::gameplay_abilities::tick_ability_tasks_system;
use crate::gameplay_effects::{
    ActiveEffectRequirementSync, ActiveEffectStorageRegistry, EffectRequirementDiagnostics,
    tick_effect_duration_system, tick_effect_period_system,
    update_active_effect_tag_requirements_system,
};
use crate::gameplay_execution::{
    GameplayExecutionQueue, GameplayExecutionResult, gameplay_execution_queue_has_work,
    process_gameplay_execution_queue_system, process_gameplay_execution_queue_with_costs_system,
};
use crate::gameplay_targeting::{
    TargetingRequestQueue, process_targeting_request_queue_system, targeting_request_queue_has_work,
};
use bevy::prelude::*;
use std::marker::PhantomData;

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
        install_runtime(app);
        app.add_systems(
            FixedUpdate,
            process_gameplay_execution_queue_system
                .run_if(gameplay_execution_queue_has_work)
                .in_set(GameplayAbilitySystemSet::GameplayResolve),
        );
    }
}

impl GameplayAbilitySystemRuntimePlugin {
    /// Creates a runtime plugin whose FIFO resolver uses external-cost provider `P`.
    ///
    /// Install this instead of the default runtime, with its supporting resource plugins.
    /// Direct calls must also use `AbilitySystemParams<P>`. Returns a plugin with the same
    /// unique plugin name as the default runtime, preventing multiple resolver installations.
    pub fn with_additional_costs<P: AdditionalCostProvider>() -> impl Plugin {
        ConfiguredRuntimePlugin::<P>(PhantomData)
    }
}

struct ConfiguredRuntimePlugin<P>(PhantomData<fn() -> P>);

impl<P: AdditionalCostProvider> Plugin for ConfiguredRuntimePlugin<P> {
    fn build(&self, app: &mut App) {
        install_runtime(app);
        app.add_systems(
            FixedUpdate,
            process_gameplay_execution_queue_with_costs_system::<P>
                .run_if(gameplay_execution_queue_has_work)
                .in_set(GameplayAbilitySystemSet::GameplayResolve),
        );
    }

    fn name(&self) -> &str {
        std::any::type_name::<GameplayAbilitySystemRuntimePlugin>()
    }
}

fn install_runtime(app: &mut App) {
    app.init_resource::<AttributeIdManager>()
        .init_resource::<GameplayExecutionQueue>()
        .add_message::<GameplayExecutionResult>()
        .init_resource::<ActiveEffectRequirementSync>()
        .init_resource::<ActiveEffectStorageRegistry>()
        .init_resource::<EffectRequirementDiagnostics>()
        .init_resource::<PendingActiveGameplayAbilities>()
        .init_resource::<TargetingRequestQueue>()
        .add_observer(cleanup_discarded_active_ability)
        .add_observer(cleanup_discarded_ability_system)
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
