use super::{
    GameplayAbilitySystemRuntimePlugin, GameplayTagPlugin, RandomPlugin, UniqueNamePlugin,
};
use crate::ability_system::AdditionalCostProvider;
use crate::gameplay_position::PositionPresentationPlugin;
use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use std::marker::PhantomData;

/// Plugin group containing GAS resources, runtime systems, and optional position presentation.
///
/// Disable [`PositionPresentationPlugin`] on the group builder for simulation-only applications
/// or when the game supplies its own presentation systems.
pub struct GameplayAbilitySystemPlugin;

impl PluginGroup for GameplayAbilitySystemPlugin {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(UniqueNamePlugin)
            .add(GameplayTagPlugin)
            .add(RandomPlugin)
            .add(GameplayAbilitySystemRuntimePlugin)
            .add(PositionPresentationPlugin)
    }
}

impl GameplayAbilitySystemPlugin {
    /// Creates the complete GAS plugin group using external-cost provider `P`.
    ///
    /// The provider declares the game's ECS reads and writes. Initialize its resources before
    /// running the schedule, and use matching typed parameters for direct calls and previews.
    /// Returns the group to install instead of `GameplayAbilitySystemPlugin`.
    pub fn with_additional_costs<P: AdditionalCostProvider>() -> impl PluginGroup {
        ConfiguredAbilitySystemPlugin::<P>(PhantomData)
    }
}

struct ConfiguredAbilitySystemPlugin<P>(PhantomData<fn() -> P>);

impl<P: AdditionalCostProvider> PluginGroup for ConfiguredAbilitySystemPlugin<P> {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(UniqueNamePlugin)
            .add(GameplayTagPlugin)
            .add(RandomPlugin)
            .add(GameplayAbilitySystemRuntimePlugin::with_additional_costs::<P>())
            .add(PositionPresentationPlugin)
    }
}
