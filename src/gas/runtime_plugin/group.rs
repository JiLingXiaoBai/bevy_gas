use super::{
    GameplayAbilitySystemRuntimePlugin, GameplayTagPlugin, RandomPlugin, UniqueNamePlugin,
};
use crate::ability_system::AdditionalCostProvider;
use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use std::marker::PhantomData;

/// Plugin group containing GAS resources and the fixed-tick execution runtime.
///
/// Games install their own spatial-query and presentation plugins independently.
pub struct GameplayAbilitySystemPlugin;

impl PluginGroup for GameplayAbilitySystemPlugin {
    fn build(self) -> PluginGroupBuilder {
        build_group::<Self>(GameplayAbilitySystemRuntimePlugin)
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
        build_group::<Self>(GameplayAbilitySystemRuntimePlugin::with_additional_costs::<P>())
    }
}

fn build_group<G: PluginGroup>(runtime: impl Plugin) -> PluginGroupBuilder {
    PluginGroupBuilder::start::<G>()
        .add(UniqueNamePlugin)
        .add(GameplayTagPlugin)
        .add(RandomPlugin)
        .add(runtime)
}
