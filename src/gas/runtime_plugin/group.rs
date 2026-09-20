use super::{
    GameplayAbilitySystemRuntimePlugin, GameplayTagPlugin, RandomPlugin, UniqueNamePlugin,
};
use crate::ability_system::AdditionalCostProvider;
use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use std::marker::PhantomData;

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
    }
}
