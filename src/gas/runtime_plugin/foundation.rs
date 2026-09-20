use crate::gameplay_tags::GameplayTagManager;
use crate::randoms::Random;
use crate::unique_names::UniqueNamePool;
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
