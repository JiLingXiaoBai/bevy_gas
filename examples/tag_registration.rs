use bevy::prelude::*;
use bevy_tools::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GameplayAbilitySystemPlugin)
        .add_systems(Startup, register_initial_tags)
        .run();
}

fn register_initial_tags(mut register: GameplayTagRegister) {
    info!("--- Registering gameplay tags ---");

    let ability_tag = match register.request_or_register_tag("Ability") {
        Ok(tag) => tag,
        Err(error) => {
            error!("Failed to register tag: {error}");
            return;
        }
    };
    if let Err(error) = register.request_or_register_tag("Effect") {
        error!("Failed to register tag: {error}");
        return;
    }
    if let Err(error) = register.request_or_register_tag("Character") {
        error!("Failed to register tag: {error}");
        return;
    }
    if let Err(error) = register.request_or_register_tag("Ability.Fireball") {
        error!("Failed to register tag: {error}");
        return;
    }
    if let Err(error) = register.request_or_register_tag("Ability.Heal") {
        error!("Failed to register tag: {error}");
        return;
    }
    let effect_debuff_stun = match register.request_or_register_tag("Effect.Debuff.Stun") {
        Ok(tag) => tag,
        Err(error) => {
            error!("Failed to register tag: {error}");
            return;
        }
    };
    if let Err(error) = register.request_or_register_tag("Effect.Buff.Speed") {
        error!("Failed to register tag: {error}");
        return;
    }

    info!("Tag registration complete.");
    info!("Ability index: {}", ability_tag.get_bit_index_usize());
    info!(
        "Effect.Debuff.Stun index: {}",
        effect_debuff_stun.get_bit_index_usize()
    );
}
