//! Headless example: two requests share one inventory bomb and only one can pay.
//! Pass a configuration directory to load ability 1002 from the Excel-authored catalog.

use bevy::ecs::system::{RunSystemOnce, SystemParam, SystemParamItem, SystemState};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_gas::config::{AbilityId, compile_catalog, load_tables};
use bevy_gas::{
    AbilityActivationCheckParams, AbilityActivationContext, AbilityTaskDef,
    AbilityTaskOnFinishedDef, AdditionalCost, AdditionalCostContext, AdditionalCostError,
    AdditionalCostProvider, AttributeIdManager, AttributeIdRegister, AttributeRegion, AttributeSet,
    EffectDurationTicks, EffectTags, GameplayAbility, GameplayAbilitySystemBundle,
    GameplayAbilitySystemPlugin, GameplayEffect, GameplayExecutionOutcome, GameplayExecutionQueue,
    GameplayExecutionResult, Modifier, ModifierMagnitude, ModifierOperation, StackingPolicy,
    can_activate_ability,
};
use bevy_gas::{UniqueName, UniqueNamePool};
use std::env;
use std::error::Error;
use std::sync::Arc;

// This game stores its authoritative bomb count in its own component.
#[derive(Component)]
struct Inventory {
    bomb_resource: UniqueName,
    bombs: u32,
}

#[derive(SystemParam)]
struct InventoryCosts<'w, 's> {
    inventories: Query<'w, 's, &'static mut Inventory>,
}

impl AdditionalCostProvider for InventoryCosts<'static, 'static> {
    type ReadOnly = Query<'static, 'static, &'static Inventory>;
    type Receipt = u32;

    fn check(
        params: &SystemParamItem<'_, '_, Self>,
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<(), AdditionalCostError> {
        if costs.is_empty() {
            return Ok(());
        }
        let inventory = params
            .inventories
            .get(context.source)
            .map_err(|_| provider_failure("ability owner has no inventory"))?;
        required_bombs(inventory, costs).map(|_| ())
    }

    fn check_readonly(
        params: &SystemParamItem<'_, '_, Self::ReadOnly>,
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<(), AdditionalCostError> {
        if costs.is_empty() {
            return Ok(());
        }
        let inventory = params
            .get(context.source)
            .map_err(|_| provider_failure("ability owner has no inventory"))?;
        required_bombs(inventory, costs).map(|_| ())
    }

    fn prepare(
        params: &mut SystemParamItem<'_, '_, Self>,
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<Self::Receipt, AdditionalCostError> {
        if costs.is_empty() {
            return Ok(0);
        }
        let mut inventory = params
            .inventories
            .get_mut(context.source)
            .map_err(|_| provider_failure("ability owner has no inventory"))?;
        let required = required_bombs(&inventory, costs)?;
        let remaining = inventory
            .bombs
            .checked_sub(required)
            .ok_or_else(|| provider_failure("inventory balance changed during preparation"))?;
        // Mutation follows validation of the complete batch. Every error leaves it unchanged.
        inventory.bombs = remaining;
        Ok(required)
    }

    fn rollback(
        params: &mut SystemParamItem<'_, '_, Self>,
        context: &AdditionalCostContext<'_>,
        receipt: Self::Receipt,
    ) -> Result<(), AdditionalCostError> {
        if receipt == 0 {
            return Ok(());
        }
        let mut inventory = params
            .inventories
            .get_mut(context.source)
            .map_err(|_| provider_failure("inventory disappeared before compensation"))?;
        let restored = inventory
            .bombs
            .checked_add(receipt)
            .ok_or_else(|| provider_failure("inventory compensation would overflow"))?;
        inventory.bombs = restored;
        Ok(())
    }
}

fn required_bombs(
    inventory: &Inventory,
    costs: &[AdditionalCost],
) -> Result<u32, AdditionalCostError> {
    let mut required = 0_u32;
    for cost in costs {
        if cost.resource() != inventory.bomb_resource {
            return Err(AdditionalCostError::UnsupportedResource {
                resource: cost.resource(),
            });
        }
        // Repeated resource identifiers are one combined requirement, not independent checks.
        required = required
            .checked_add(cost.amount())
            .ok_or_else(|| provider_failure("combined bomb cost would overflow"))?;
    }
    if inventory.bombs < required {
        return Err(AdditionalCostError::InsufficientResource {
            resource: inventory.bomb_resource,
            required,
            available: inventory.bombs,
        });
    }
    Ok(required)
}

fn provider_failure(reason: &'static str) -> AdditionalCostError {
    AdditionalCostError::ProviderFailure {
        reason: reason.into(),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, LogPlugin::default()))
        .add_plugins(GameplayAbilitySystemPlugin::with_additional_costs::<
            InventoryCosts,
        >());

    let bomb = app
        .world_mut()
        .get_resource_mut::<UniqueNamePool>()
        .ok_or("missing unique-name pool")?
        .new_name("Inventory.Bomb")?;
    let (mana, ability, initial_mana, expected_mana) = if let Some(directory) = env::args().nth(1) {
        let tables = load_tables(directory)?;
        let catalog = compile_catalog(&tables, app.world_mut())?;
        let mana = catalog.attribute("Mana").ok_or("sample requires Mana")?;
        let ability = Arc::clone(
            catalog
                .ability(AbilityId(1002))
                .ok_or("sample requires configured ability 1002")?
                .definition(),
        );
        app.world_mut().insert_resource(catalog);
        (mana, ability, 40.0, 20.0)
    } else {
        let mana = app
            .world_mut()
            .run_system_once(|mut register: AttributeIdRegister| {
                register.request_or_register_attribute_id("Mana", AttributeRegion::Hot)
            })
            .map_err(|error| format!("attribute registration system failed: {error}"))??;

        let mana_cost = Arc::new(GameplayEffect::new(
            vec![Modifier::new(
                mana,
                ModifierOperation::Add,
                ModifierMagnitude::Flat(-10.0),
            )],
            EffectDurationTicks::Instant,
            None,
            1.0,
            StackingPolicy::non_stacking(),
            EffectTags::new(Vec::new(), Vec::new()),
        ));
        let ability = Arc::new(
            GameplayAbility::default()
                .with_cost(mana_cost)
                .with_additional_costs(vec![AdditionalCost::new(bomb, 1)?])
                .with_allow_multiple_instances(true)
                .with_startup_tasks(vec![AbilityTaskDef::instant(
                    AbilityTaskOnFinishedDef::EndAbility,
                )]),
        );
        (mana, ability, 20.0, 10.0)
    };
    let mut bundle = GameplayAbilitySystemBundle::default();
    let manager = app
        .world()
        .get_resource::<AttributeIdManager>()
        .ok_or("missing attribute registry")?;
    bundle
        .attributes
        .initialize_attribute(manager, mana, initial_mana, None)?;
    let handle = bundle.ability_system.give_ability(Arc::clone(&ability), 1);
    let source = app
        .world_mut()
        .spawn((
            bundle,
            Inventory {
                bomb_resource: bomb,
                bombs: 1,
            },
        ))
        .id();

    // UI and AI previews use the same provider with genuinely read-only ECS access.
    let mut preview =
        SystemState::<AbilityActivationCheckParams<InventoryCosts>>::new(app.world_mut());
    can_activate_ability(
        source,
        source,
        &ability,
        1,
        &preview
            .get(app.world())
            .map_err(|error| format!("preview parameters are unavailable: {error}"))?,
    )?;

    {
        let mut queue = app
            .world_mut()
            .get_resource_mut::<GameplayExecutionQueue>()
            .ok_or("missing gameplay execution queue")?;
        for _ in 0..2 {
            let context = AbilityActivationContext::direct(source, queue.new_root_chain(handle));
            queue.push_activation(source, source, handle, context)?;
        }
    }
    app.world_mut().run_schedule(FixedUpdate);

    let results = app
        .world()
        .get_resource::<Messages<GameplayExecutionResult>>()
        .ok_or("missing gameplay execution results")?;
    let mut successes = 0;
    let mut rejections = 0;
    for result in results.get_cursor().read(results) {
        match &result.outcome {
            GameplayExecutionOutcome::Succeeded => successes += 1,
            GameplayExecutionOutcome::Rejected(error) => {
                rejections += 1;
                info!(%error, "Second bomb request was rejected");
            }
            GameplayExecutionOutcome::Failed(error) => return Err(error.clone().into()),
        }
    }
    let bombs_remaining = app
        .world()
        .get::<Inventory>(source)
        .ok_or("missing source inventory")?
        .bombs;
    let mut attributes =
        SystemState::<(Res<AttributeIdManager>, Query<&mut AttributeSet>)>::new(app.world_mut());
    let (manager, mut query) = attributes
        .get_mut(app.world_mut())
        .map_err(|error| format!("attribute parameters are unavailable: {error}"))?;
    let mana_remaining = query
        .get_mut(source)?
        .get_current_value(&manager, mana)?
        .ok_or("missing source mana")?;
    if successes != 1 || rejections != 1 || bombs_remaining != 0 || mana_remaining != expected_mana
    {
        return Err(
            "expected one paid bomb activation and one rejection without a second mana debit"
                .into(),
        );
    }
    info!(
        successes,
        rejections, bombs_remaining, mana_remaining, "One fixed tick completed"
    );
    Ok(())
}
