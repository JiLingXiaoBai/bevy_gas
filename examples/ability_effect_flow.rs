//! Runs one complete fireball cast without a window or wall-clock delays.
//!
//! Run with `cargo run --example ability_effect_flow`. Tick zero is the activation
//! tick: mana is spent immediately, damage lands at tick five, the ability ends
//! at tick six, and its independent cooldown expires at tick twenty.

use bevy::ecs::system::RunSystemOnce;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_gas::gas::attributes::AttributeIdManager;
use bevy_gas::gas::gameplay_abilities::{AbilityTaskDef, AbilityTaskOnFinishedDef};
use bevy_gas::prelude::*;
use std::error::Error;
use std::sync::Arc;

type ExampleResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Resource)]
struct GameplayIds {
    health: AttributeId,
    mana: AttributeId,
    fireball: GameplayTag,
    cooldown: GameplayTag,
}

#[derive(Resource)]
struct FireballScenario {
    caster: Entity,
    target: Entity,
    ability: AbilitySpecHandle,
    submitted: bool,
}

#[derive(Resource, Default)]
struct ExampleTick(u32);

fn main() -> ExampleResult<()> {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        LogPlugin::default(),
        GameplayAbilitySystemPlugin,
    ))
    .init_resource::<ExampleTick>()
    .add_systems(
        FixedUpdate,
        queue_fireball.in_set(GameplayAbilitySystemSet::RequestProducers),
    );
    app.finish();
    app.cleanup();

    // Keep registration and actor initialization ordered, just as chained
    // Startup systems would be in a game. Propagate setup failures to main.
    let (health, mana) = app
        .world_mut()
        .run_system_once(register_attributes)
        .map_err(|error| format!("attribute registration system failed: {error:?}"))??;
    let (fireball, cooldown) = app
        .world_mut()
        .run_system_once(register_tags)
        .map_err(|error| format!("tag registration system failed: {error:?}"))??;
    app.insert_resource(GameplayIds {
        health,
        mana,
        fireball,
        cooldown,
    });
    app.world_mut()
        .run_system_once(spawn_actors)
        .map_err(|error| format!("actor initialization system failed: {error:?}"))??;

    info!("Before casting: mana=50, target_health=100, no active ability or cooldown");
    // Drive the real plugin schedule directly so the example finishes quickly
    // and produces the same gameplay timeline regardless of rendering speed.
    for tick in 0..=20 {
        app.world_mut().resource_mut::<ExampleTick>().0 = tick;
        app.world_mut().run_schedule(FixedUpdate);
        if matches!(tick, 0 | 5 | 6 | 20) {
            app.world_mut()
                .run_system_once(report_state)
                .map_err(|error| format!("state reporting system failed: {error:?}"))??;
        }
    }
    Ok(())
}

fn register_attributes(
    mut attributes: AttributeIdRegister,
) -> ExampleResult<(AttributeId, AttributeId)> {
    let health = attributes.request_or_register_attribute_id("Health", AttributeRegion::Hot)?;
    let mana = attributes.request_or_register_attribute_id("Mana", AttributeRegion::Hot)?;
    Ok((health, mana))
}

fn register_tags(mut tags: GameplayTagRegister) -> ExampleResult<(GameplayTag, GameplayTag)> {
    let fireball = tags.request_or_register_tag("Ability.Fireball")?;
    let cooldown = tags.request_or_register_tag("Cooldown.Fireball")?;
    Ok((fireball, cooldown))
}

fn spawn_actors(
    mut commands: Commands,
    ids: Res<GameplayIds>,
    manager: Res<AttributeIdManager>,
) -> ExampleResult<()> {
    let mut caster = GameplayAbilitySystemBundle::default();
    caster
        .attributes
        .initialize_attribute(&manager, ids.mana, 50.0, None)?;
    let ability = caster.ability_system.give_ability(make_fireball(&ids), 1);

    let mut target = GameplayAbilitySystemBundle::default();
    target
        .attributes
        .initialize_attribute(&manager, ids.health, 100.0, None)?;

    let caster = commands.spawn(caster).id();
    let target = commands.spawn(target).id();
    commands.insert_resource(FireballScenario {
        caster,
        target,
        ability,
        submitted: false,
    });
    Ok(())
}

fn make_fireball(ids: &GameplayIds) -> Arc<GameplayAbility> {
    let cost = instant_delta(ids.mana, -20.0);
    let damage = instant_delta(ids.health, -30.0);
    let cooldown = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(20.0)),
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(Vec::new(), vec![ids.cooldown]),
    ));

    Arc::new(
        GameplayAbility::default()
            .with_tags(AbilityTags::default().with_ability_asset_tags(vec![ids.fireball]))
            .with_cost(cost)
            .with_cooldown(cooldown)
            .with_startup_tasks(vec![
                AbilityTaskDef::wait_ticks(
                    5,
                    AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget { effect: damage },
                ),
                AbilityTaskDef::wait_ticks(6, AbilityTaskOnFinishedDef::EndAbility),
            ]),
    )
}

fn instant_delta(attribute: AttributeId, value: f32) -> Arc<GameplayEffect> {
    Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            attribute,
            ModifierOperation::Add,
            ModifierMagnitude::Flat(value),
        )],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(Vec::new(), Vec::new()),
    ))
}

fn queue_fireball(
    mut scenario: ResMut<FireballScenario>,
    mut queue: ResMut<GameplayExecutionQueue>,
) {
    if scenario.submitted {
        return;
    }
    let chain = queue.new_root_chain(scenario.ability);
    let context = AbilityActivationContext::direct(scenario.caster, chain);
    queue.push_activation(scenario.caster, scenario.target, scenario.ability, context);
    scenario.submitted = true;
}

fn report_state(
    tick: Res<ExampleTick>,
    scenario: Res<FireballScenario>,
    ids: Res<GameplayIds>,
    manager: Res<AttributeIdManager>,
    mut attributes: Query<&mut AttributeSet>,
    tags: Query<&GameplayTagContainer>,
    abilities: Query<&AbilitySystemComponent>,
) -> ExampleResult<()> {
    let mana = attributes
        .get_mut(scenario.caster)?
        .get_current_value(&manager, ids.mana)?
        .ok_or("caster Mana was not initialized")?;
    let health = attributes
        .get_mut(scenario.target)?
        .get_current_value(&manager, ids.health)?
        .ok_or("target Health was not initialized")?;
    let active_count = abilities
        .get(scenario.caster)?
        .find_ability_spec(scenario.ability)
        .ok_or("fireball was not granted")?
        .get_active_count();
    let on_cooldown = tags.get(scenario.caster)?.has_tag(&ids.cooldown);
    info!(
        tick = tick.0,
        mana,
        target_health = health,
        active_count,
        on_cooldown,
        "Fireball state after FixedUpdate"
    );
    Ok(())
}
