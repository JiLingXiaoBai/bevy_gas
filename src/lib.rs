mod gas;
mod randoms;
mod unique_names;

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
pub use gas::*;
pub use randoms::*;
pub use unique_names::*;
extern crate core;

pub struct GameplayTagPlugin;

impl Plugin for GameplayTagPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameplayTagManager>();
    }
}

pub struct UniqueNamePlugin;

impl Plugin for UniqueNamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UniqueNamePool>();
    }
}

pub struct RandomPlugin;

impl Plugin for RandomPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Random>();
    }
}

pub struct GameplayAbilitySystemRuntimePlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum GameplayAbilitySystemSet {
    UpdateEffectTagRequirements,
    EffectTicks,
    AbilityTasks,
    Queues,
    Cleanup,
    RecalculateAttributes,
}

impl Plugin for GameplayAbilitySystemRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AttributeIdManager>()
            .init_resource::<AbilityActivationQueue>()
            .init_resource::<GameplayEffectApplicationQueue>()
            .init_resource::<ActiveGameplayEffectTargetIndex>()
            .configure_sets(
                FixedUpdate,
                (
                    GameplayAbilitySystemSet::UpdateEffectTagRequirements
                        .before(GameplayAbilitySystemSet::EffectTicks),
                    GameplayAbilitySystemSet::EffectTicks.before(GameplayAbilitySystemSet::Queues),
                    GameplayAbilitySystemSet::AbilityTasks.before(GameplayAbilitySystemSet::Queues),
                    GameplayAbilitySystemSet::Queues.before(GameplayAbilitySystemSet::Cleanup),
                    GameplayAbilitySystemSet::Cleanup
                        .before(GameplayAbilitySystemSet::RecalculateAttributes),
                ),
            )
            .add_systems(
                FixedUpdate,
                update_active_effect_tag_requirements_system
                    .in_set(GameplayAbilitySystemSet::UpdateEffectTagRequirements),
            )
            .add_systems(
                FixedUpdate,
                (tick_effect_duration_system, tick_effect_period_system)
                    .chain()
                    .in_set(GameplayAbilitySystemSet::EffectTicks),
            )
            .add_systems(
                FixedUpdate,
                tick_ability_tasks_system.in_set(GameplayAbilitySystemSet::AbilityTasks),
            )
            .add_systems(
                FixedUpdate,
                (
                    process_gameplay_effect_application_queue_system
                        .run_if(gameplay_effect_application_queue_has_work),
                    process_ability_activation_queue_system
                        .run_if(ability_activation_queue_has_work),
                )
                    .chain()
                    .in_set(GameplayAbilitySystemSet::Queues),
            )
            .add_systems(
                FixedUpdate,
                (
                    cleanup_finished_abilities_system,
                    reconcile_active_effect_target_index_system,
                )
                    .chain()
                    .in_set(GameplayAbilitySystemSet::Cleanup),
            )
            .add_systems(
                FixedUpdate,
                recalculate_attribute_sets_system
                    .in_set(GameplayAbilitySystemSet::RecalculateAttributes),
            );
    }
}

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
