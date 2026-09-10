//! Headless execution of the real Excel-authored fireball through the GAS queues.

use bevy::prelude::*;
use bevy_gas::config::{
    AbilityId, ConfiguredAbilities, compile_catalog, grant_ability, load_tables,
};
use bevy_gas::{
    AbilityActivationContext, ActiveGameplayAbility, AttributeIdManager, AttributeSet,
    GameplayAbilitySystemBundle, GameplayAbilitySystemPlugin, GameplayExecutionQueue, Targetable,
    TargetingContinuation, TargetingInput, TargetingRequestQueue,
};
use std::env;
use std::error::Error;
use std::io::{self, Write};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn Error>> {
    let directory = env::args().nth(1).ok_or(
        "usage: cargo run --features luban-config --example config_fireball -- <data-directory>",
    )?;
    let tables = load_tables(directory)?;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(GameplayAbilitySystemPlugin);
    let catalog = compile_catalog(&tables, app.world_mut())?;
    let health = catalog
        .attribute("Health")
        .ok_or("sample requires Health")?;
    let mana = catalog.attribute("Mana").ok_or("sample requires Mana")?;
    let mut source_bundle = GameplayAbilitySystemBundle::default();
    let mut target_bundle = GameplayAbilitySystemBundle::default();
    let manager = app
        .world()
        .get_resource::<AttributeIdManager>()
        .ok_or("missing attribute registry")?;
    source_bundle
        .attributes
        .initialize_attribute(manager, mana, 100.0, None)?;
    source_bundle
        .attributes
        .initialize_attribute(manager, health, 500.0, None)?;
    target_bundle
        .attributes
        .initialize_attribute(manager, mana, 100.0, None)?;
    target_bundle
        .attributes
        .initialize_attribute(manager, health, 500.0, None)?;
    let mut bindings = ConfiguredAbilities::default();
    let handle = grant_ability(
        &mut source_bundle.ability_system,
        &mut bindings,
        &catalog,
        AbilityId(1001),
        5,
    )?;
    let targeting = Arc::clone(
        catalog
            .ability(AbilityId(1001))
            .ok_or("missing sample ability")?
            .targeting(),
    );
    let source = app
        .world_mut()
        .spawn((source_bundle, bindings, GlobalTransform::IDENTITY))
        .id();
    let target = app
        .world_mut()
        .spawn((
            target_bundle,
            Targetable,
            GlobalTransform::from_translation(Vec3::X * 5.0),
        ))
        .id();
    let chain = app
        .world_mut()
        .get_resource_mut::<GameplayExecutionQueue>()
        .ok_or("missing execution queue")?
        .new_root_chain(handle);
    app.world_mut()
        .get_resource_mut::<TargetingRequestQueue>()
        .ok_or("missing targeting queue")?
        .push_request(
            source,
            TargetingInput::new(Vec3::ZERO, Vec3::X).with_explicit_target(target),
            targeting,
            TargetingContinuation::activate_ability(
                handle,
                AbilityActivationContext::direct(source, chain),
            ),
        );
    app.world_mut().insert_resource(catalog);
    app.world_mut().run_schedule(FixedUpdate);
    for _ in 0..12 {
        app.world_mut().run_schedule(FixedUpdate);
    }
    let manager = app
        .world()
        .get_resource::<AttributeIdManager>()
        .ok_or("missing attribute registry")?
        .clone();
    let health_value = app
        .world_mut()
        .get_mut::<AttributeSet>(target)
        .ok_or("missing target attributes")?
        .get_current_value(&manager, health)?
        .ok_or("missing target Health")?;
    let mana_value = app
        .world_mut()
        .get_mut::<AttributeSet>(source)
        .ok_or("missing source attributes")?
        .get_current_value(&manager, mana)?
        .ok_or("missing source Mana")?;
    let active = app
        .world_mut()
        .query::<&ActiveGameplayAbility>()
        .iter(app.world())
        .count();
    writeln!(
        io::stdout().lock(),
        "Fireball level 5 after 12 ticks: target Health={health_value}, source Mana={mana_value}, active abilities={active}"
    )?;
    if health_value != 320.0 || mana_value != 80.0 || active != 0 {
        return Err("sample did not produce the expected configured gameplay result".into());
    }
    Ok(())
}
