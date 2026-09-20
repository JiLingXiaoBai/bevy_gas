use super::*;
use bevy::ecs::system::{RunSystemOnce, SystemParam, SystemParamItem, SystemState};
use bevy_gas::{
    AbilityActivationCheckError, AbilityActivationCheckParams, AbilityCommitError,
    AbilitySystemParams, AdditionalCost, AdditionalCostContext, AdditionalCostError,
    AdditionalCostProvider, AttributeIdManager, AttributeSet, GameplayAbilitySystemPlugin,
    GameplayAbilitySystemRuntimePlugin, GameplayEffectApplicationError, GameplayExecutionOutcome,
    GameplayExecutionResult, UniqueName, UniqueNamePool, can_activate_ability, cancel_ability,
    commit_ability, try_activate_ability_by_handle,
};

#[derive(Component, Clone, Debug, PartialEq, Eq)]
struct Inventory {
    balances: Vec<(UniqueName, u32)>,
}

impl Inventory {
    fn planned_debit(&self, costs: &[AdditionalCost]) -> Result<Self, AdditionalCostError> {
        let mut totals: Vec<(UniqueName, u32)> = Vec::new();
        for cost in costs {
            if let Some((_, total)) = totals.iter_mut().find(|(id, _)| *id == cost.resource()) {
                *total = total.checked_add(cost.amount()).ok_or_else(|| {
                    AdditionalCostError::ProviderFailure {
                        reason: "inventory cost total overflow".into(),
                    }
                })?;
            } else {
                totals.push((cost.resource(), cost.amount()));
            }
        }
        let mut result = self.clone();
        for (resource, required) in totals {
            let (_, available) = result
                .balances
                .iter_mut()
                .find(|(id, _)| *id == resource)
                .ok_or(AdditionalCostError::UnsupportedResource { resource })?;
            if *available < required {
                return Err(AdditionalCostError::InsufficientResource {
                    resource,
                    required,
                    available: *available,
                });
            }
            *available -= required;
        }
        Ok(result)
    }
}

#[derive(Resource, Default)]
struct ProviderLog {
    prepare_calls: usize,
    rollback_calls: usize,
    reject_prepare: bool,
    fail_rollback: bool,
    contexts: Vec<(Entity, u32, Option<AbilityActivationReason>)>,
}

#[derive(SystemParam)]
struct InventoryProvider<'w, 's> {
    inventories: Query<'w, 's, &'static mut Inventory>,
    log: ResMut<'w, ProviderLog>,
}

type InventoryCosts = InventoryProvider<'static, 'static>;

impl AdditionalCostProvider for InventoryCosts {
    type ReadOnly = Query<'static, 'static, &'static Inventory>;
    type Receipt = Inventory;

    fn check(
        params: &SystemParamItem<'_, '_, Self>,
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<(), AdditionalCostError> {
        let inventory = params.inventories.get(context.source).map_err(|_| {
            AdditionalCostError::ProviderFailure {
                reason: "inventory is missing".into(),
            }
        })?;
        inventory.planned_debit(costs).map(|_| ())
    }

    fn check_readonly(
        params: &SystemParamItem<'_, '_, Self::ReadOnly>,
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<(), AdditionalCostError> {
        let inventory =
            params
                .get(context.source)
                .map_err(|_| AdditionalCostError::ProviderFailure {
                    reason: "inventory is missing".into(),
                })?;
        inventory.planned_debit(costs).map(|_| ())
    }

    fn prepare(
        params: &mut SystemParamItem<'_, '_, Self>,
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<Self::Receipt, AdditionalCostError> {
        params.log.prepare_calls += 1;
        params.log.contexts.push((
            context.source,
            context.level,
            context.activation.map(AbilityActivationContext::get_reason),
        ));
        if params.log.reject_prepare {
            return Err(AdditionalCostError::Rejected {
                reason: "inventory is locked".into(),
            });
        }
        let mut inventory = params.inventories.get_mut(context.source).map_err(|_| {
            AdditionalCostError::ProviderFailure {
                reason: "inventory is missing".into(),
            }
        })?;
        let debited = inventory.planned_debit(costs)?;
        Ok(std::mem::replace(&mut *inventory, debited))
    }

    fn rollback(
        params: &mut SystemParamItem<'_, '_, Self>,
        context: &AdditionalCostContext<'_>,
        receipt: Self::Receipt,
    ) -> Result<(), AdditionalCostError> {
        params.log.rollback_calls += 1;
        if params.log.fail_rollback {
            return Err(AdditionalCostError::ProviderFailure {
                reason: "inventory compensation failed".into(),
            });
        }
        let mut inventory = params.inventories.get_mut(context.source).map_err(|_| {
            AdditionalCostError::ProviderFailure {
                reason: "inventory is missing".into(),
            }
        })?;
        *inventory = receipt;
        Ok(())
    }
}

fn inventory_app() -> App {
    let mut app = App::new();
    app.init_resource::<ProviderLog>().add_plugins((
        MinimalPlugins,
        GameplayAbilitySystemPlugin::with_additional_costs::<InventoryCosts>(),
    ));
    app
}

fn resource_name(app: &mut App, name: &str) -> UniqueName {
    app.world_mut()
        .resource_mut::<UniqueNamePool>()
        .new_name(name)
        .unwrap()
}

fn spawn_inventory_owner(app: &mut App, resource: UniqueName, amount: u32) -> Entity {
    app.world_mut()
        .spawn((
            GameplayAbilitySystemBundle::default(),
            Inventory {
                balances: vec![(resource, amount)],
            },
        ))
        .id()
}

fn balance(app: &App, source: Entity, resource: UniqueName) -> u32 {
    app.world()
        .get::<Inventory>(source)
        .unwrap()
        .balances
        .iter()
        .find(|(id, _)| *id == resource)
        .unwrap()
        .1
}

fn item_ability(resource: UniqueName, amount: u32) -> GameplayAbility {
    GameplayAbility::default()
        .with_additional_costs(vec![AdditionalCost::new(resource, amount).unwrap()])
}

fn activate_with_inventory(
    app: &mut App,
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
) -> Result<(), AbilityActivationError> {
    app.world_mut()
        .run_system_once(move |mut params: AbilitySystemParams<InventoryCosts>| {
            try_activate_ability_by_handle(
                source,
                target,
                handle,
                AbilityActivationContext::direct(source, AbilityChainContext::root(handle, 0)),
                &mut params,
            )
        })
        .unwrap()
}

fn commit_with_inventory(
    app: &mut App,
    source: Entity,
    ability: Arc<GameplayAbility>,
) -> Result<(), AbilityCommitError> {
    app.world_mut()
        .run_system_once(move |mut params: AbilitySystemParams<InventoryCosts>| {
            commit_ability(source, &ability, 7, &mut params)
        })
        .unwrap()
}

#[test]
fn additional_cost_requires_a_positive_amount() {
    let mut app = inventory_app();
    let bomb = resource_name(&mut app, "Item.Bomb");
    assert_eq!(
        AdditionalCost::new(bomb, 0),
        Err(AdditionalCostError::InvalidAmount)
    );
    let cost = AdditionalCost::new(bomb, 3).unwrap();
    assert_eq!(cost.resource(), bomb);
    assert_eq!(cost.amount(), 3);
}

#[test]
fn direct_activation_charges_the_owner_once_and_preserves_context() {
    let mut app = inventory_app();
    let bomb = resource_name(&mut app, "Item.Bomb");
    let source = spawn_inventory_owner(&mut app, bomb, 3);
    let target = spawn_inventory_owner(&mut app, bomb, 8);
    let handle = app
        .world_mut()
        .get_mut::<AbilitySystemComponent>(source)
        .unwrap()
        .give_ability(Arc::new(item_ability(bomb, 2)), 4);

    assert_eq!(
        activate_with_inventory(&mut app, source, target, handle),
        Ok(())
    );
    assert_eq!(balance(&app, source, bomb), 1);
    assert_eq!(balance(&app, target, bomb), 8);
    let log = app.world().resource::<ProviderLog>();
    assert_eq!(log.prepare_calls, 1);
    assert_eq!(log.rollback_calls, 0);
    assert_eq!(
        log.contexts,
        vec![(source, 4, Some(AbilityActivationReason::Direct))]
    );
}

#[test]
fn readonly_precheck_uses_immutable_world_without_spending_inventory() {
    let mut app = inventory_app();
    let bomb = resource_name(&mut app, "Item.Bomb");
    let source = spawn_inventory_owner(&mut app, bomb, 1);
    let affordable = Arc::new(item_ability(bomb, 1));
    let expensive = Arc::new(item_ability(bomb, 2));
    let mut state =
        SystemState::<AbilityActivationCheckParams<InventoryCosts>>::new(app.world_mut());
    let params = state.get(app.world()).unwrap();
    assert_eq!(
        can_activate_ability(source, source, &affordable, 1, &params),
        Ok(())
    );
    assert_eq!(
        can_activate_ability(source, source, &expensive, 1, &params),
        Err(AbilityActivationCheckError::Cost(
            AbilityCommitError::AdditionalCost(AdditionalCostError::InsufficientResource {
                resource: bomb,
                required: 2,
                available: 1
            })
        ))
    );
    assert_eq!(balance(&app, source, bomb), 1);
    assert_eq!(app.world().resource::<ProviderLog>().prepare_calls, 0);
}

#[test]
fn standalone_commit_pays_inventory_without_starting_an_instance() {
    let mut app = inventory_app();
    let bomb = resource_name(&mut app, "Item.Bomb");
    let source = spawn_inventory_owner(&mut app, bomb, 1);
    assert_eq!(
        commit_with_inventory(&mut app, source, Arc::new(item_ability(bomb, 1))),
        Ok(())
    );
    assert_eq!(balance(&app, source, bomb), 0);
    assert_eq!(active_ability_count(&mut app), 0);
    assert_eq!(
        app.world().resource::<ProviderLog>().contexts,
        vec![(source, 7, None)]
    );
}

#[test]
fn fifo_requests_compete_for_the_last_item_at_resolution() {
    let mut app = inventory_app();
    let bomb = resource_name(&mut app, "Item.Bomb");
    let source = spawn_inventory_owner(&mut app, bomb, 1);
    let handle = give_ability(
        &mut app,
        source,
        Arc::new(item_ability(bomb, 1).with_allow_multiple_instances(true)),
    );
    let ids = {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        let mut ids = Vec::new();
        for _ in 0..2 {
            let context = AbilityActivationContext::direct(source, queue.new_root_chain(handle));
            ids.push(
                queue
                    .push_activation(source, source, handle, context)
                    .unwrap(),
            );
        }
        ids
    };
    app.world_mut().run_schedule(FixedUpdate);
    let results: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<GameplayExecutionResult>>()
        .drain()
        .collect();
    assert_eq!(
        results
            .iter()
            .map(|result| result.request_id)
            .collect::<Vec<_>>(),
        ids
    );
    assert_eq!(results[0].outcome, GameplayExecutionOutcome::Succeeded);
    assert!(matches!(
        results[1].outcome,
        GameplayExecutionOutcome::Rejected(_)
    ));
    assert_eq!(balance(&app, source, bomb), 0);
    assert_eq!(active_ability_count(&mut app), 1);
    assert_eq!(app.world().resource::<ProviderLog>().prepare_calls, 1);
}

#[test]
fn repeated_costs_are_checked_as_a_batch_and_debited_once() {
    for (available, succeeds) in [(5, false), (6, true)] {
        let mut app = inventory_app();
        let bomb = resource_name(&mut app, "Item.Bomb");
        let source = spawn_inventory_owner(&mut app, bomb, available);
        let ability = Arc::new(GameplayAbility::default().with_additional_costs(vec![
            AdditionalCost::new(bomb, 3).unwrap(),
            AdditionalCost::new(bomb, 3).unwrap(),
        ]));
        let result = commit_with_inventory(&mut app, source, ability);
        if succeeds {
            assert_eq!(result, Ok(()));
            assert_eq!(balance(&app, source, bomb), 0);
        } else {
            assert_eq!(
                result,
                Err(AbilityCommitError::AdditionalCost(
                    AdditionalCostError::InsufficientResource {
                        resource: bomb,
                        required: 6,
                        available
                    }
                ))
            );
            assert_eq!(balance(&app, source, bomb), available);
            assert_eq!(app.world().resource::<ProviderLog>().prepare_calls, 0);
        }
    }
}

#[test]
fn a_later_unaffordable_resource_does_not_spend_earlier_items_or_attributes() {
    let mut app = inventory_app();
    let bomb = resource_name(&mut app, "Item.Bomb");
    let reagent = resource_name(&mut app, "Item.Reagent");
    let mana = register_attribute(&mut app, "Mana");
    let source = spawn_inventory_owner(&mut app, bomb, 2);
    app.world_mut()
        .get_mut::<Inventory>(source)
        .unwrap()
        .balances
        .push((reagent, 0));
    let attributes = attribute_set(&app, mana, 10.0);
    app.world_mut().entity_mut(source).insert(attributes);
    let ability = Arc::new(
        GameplayAbility::default()
            .with_additional_costs(vec![
                AdditionalCost::new(bomb, 1).unwrap(),
                AdditionalCost::new(reagent, 1).unwrap(),
            ])
            .with_cost(instant_add_effect(mana, -3.0)),
    );
    assert_eq!(
        commit_with_inventory(&mut app, source, ability),
        Err(AbilityCommitError::AdditionalCost(
            AdditionalCostError::InsufficientResource {
                resource: reagent,
                required: 1,
                available: 0
            }
        ))
    );
    assert_eq!(balance(&app, source, bomb), 2);
    assert_eq!(balance(&app, source, reagent), 0);
    assert_eq!(current_value(&mut app, source, mana), 10.0);
    assert_eq!(app.world().resource::<ProviderLog>().prepare_calls, 0);
}

#[test]
fn prepare_rejection_leaves_attributes_and_inventory_unchanged() {
    let mut app = inventory_app();
    let bomb = resource_name(&mut app, "Item.Bomb");
    let mana = register_attribute(&mut app, "Mana");
    let source = spawn_inventory_owner(&mut app, bomb, 2);
    let attributes = attribute_set(&app, mana, 10.0);
    app.world_mut().entity_mut(source).insert(attributes);
    app.world_mut().resource_mut::<ProviderLog>().reject_prepare = true;
    let handle = give_ability(
        &mut app,
        source,
        Arc::new(item_ability(bomb, 1).with_cost(instant_add_effect(mana, -3.0))),
    );
    let error = activate_with_inventory(&mut app, source, source, handle).unwrap_err();
    assert_eq!(
        error,
        AbilityActivationError::CommitExecutionFailed {
            source,
            handle,
            error: AbilityCommitError::AdditionalCost(AdditionalCostError::Rejected {
                reason: "inventory is locked".into(),
            }),
        }
    );
    assert!(error.is_rejection());
    assert_eq!(balance(&app, source, bomb), 2);
    assert_eq!(current_value(&mut app, source, mana), 10.0);
    assert_eq!(active_ability_count(&mut app), 0);
    let log = app.world().resource::<ProviderLog>();
    assert_eq!((log.prepare_calls, log.rollback_calls), (1, 0));
}

fn erase_attributes_after_payment(
    attributes: &mut AttributeSet,
    _manager: &AttributeIdManager,
    _attribute: AttributeId,
    _old_value: f32,
    _new_value: f32,
) {
    *attributes = AttributeSet::default();
}

#[test]
fn gas_execution_failure_compensates_inventory_and_retains_the_original_error() {
    for fail_rollback in [false, true] {
        let mut app = inventory_app();
        let bomb = resource_name(&mut app, "Item.Bomb");
        let mana = register_attribute(&mut app, "Mana");
        let source = spawn_inventory_owner(&mut app, bomb, 1);
        let mut attributes = attribute_set(&app, mana, 10.0);
        attributes.set_post_execute(Some(erase_attributes_after_payment));
        app.world_mut().entity_mut(source).insert(attributes);
        app.world_mut().resource_mut::<ProviderLog>().fail_rollback = fail_rollback;
        let ability = Arc::new(
            item_ability(bomb, 1)
                .with_cost(instant_add_effect(mana, -1.0))
                .with_cooldown(instant_add_effect(mana, -1.0)),
        );
        let handle = give_ability(&mut app, source, ability);
        let original = AbilityCommitError::CooldownExecution(
            GameplayEffectApplicationError::MissingAttribute {
                target: source,
                id: mana,
            },
        );
        let expected = if fail_rollback {
            AbilityCommitError::AdditionalCostRollback {
                commit_error: Box::new(original),
                rollback_error: AdditionalCostError::ProviderFailure {
                    reason: "inventory compensation failed".into(),
                },
            }
        } else {
            original
        };
        let error = activate_with_inventory(&mut app, source, source, handle).unwrap_err();
        assert_eq!(
            error,
            AbilityActivationError::CommitExecutionFailed {
                source,
                handle,
                error: expected
            }
        );
        assert!(!error.is_rejection());
        assert_eq!(balance(&app, source, bomb), u32::from(!fail_rollback));
        assert_eq!(active_ability_count(&mut app), 0);
        let log = app.world().resource::<ProviderLog>();
        assert_eq!((log.prepare_calls, log.rollback_calls), (1, 1));
    }
}

#[test]
fn chained_activation_uses_the_same_provider_and_sees_parent_payment() {
    for available in [1, 2] {
        let mut app = inventory_app();
        let bomb = resource_name(&mut app, "Item.Bomb");
        let source = spawn_inventory_owner(&mut app, bomb, available);
        let child = give_ability(&mut app, source, Arc::new(item_ability(bomb, 1)));
        let parent = give_ability(
            &mut app,
            source,
            Arc::new(
                item_ability(bomb, 1).with_startup_tasks(vec![AbilityTaskDef::instant(
                    AbilityTaskOnFinishedDef::ActivateAbility { handle: child },
                )]),
            ),
        );
        assert_eq!(
            activate_with_inventory(&mut app, source, source, parent),
            Ok(())
        );
        assert_eq!(balance(&app, source, bomb), 0);
        assert_eq!(active_ability_count(&mut app), available as usize);
        assert_eq!(
            app.world().resource::<ProviderLog>().prepare_calls,
            available as usize
        );
        if available == 2 {
            assert!(matches!(
                app.world().resource::<ProviderLog>().contexts[1].2,
                Some(AbilityActivationReason::Chained { .. })
            ));
        }
    }
}

#[test]
fn missing_provider_rejects_additional_costs_instead_of_granting_a_free_activation() {
    let mut app = test_app();
    let bomb = resource_name(&mut app, "Item.Bomb");
    let source = spawn_inventory_owner(&mut app, bomb, 1);
    let handle = give_ability(&mut app, source, Arc::new(item_ability(bomb, 1)));
    assert_eq!(
        activate_ability_result(&mut app, source, source, handle),
        Err(AbilityActivationError::CommitPreparationFailed {
            source,
            handle,
            error: AbilityCommitError::AdditionalCost(AdditionalCostError::MissingProvider),
        })
    );
    assert_eq!(balance(&app, source, bomb), 1);
    assert_eq!(active_ability_count(&mut app), 0);
}

#[test]
fn unsupported_resources_fail_closed_without_spending_known_items() {
    let mut app = inventory_app();
    let bomb = resource_name(&mut app, "Item.Bomb");
    let unsupported = resource_name(&mut app, "Item.Unknown");
    let source = spawn_inventory_owner(&mut app, bomb, 1);
    let ability = Arc::new(GameplayAbility::default().with_additional_costs(vec![
        AdditionalCost::new(bomb, 1).unwrap(),
        AdditionalCost::new(unsupported, 1).unwrap(),
    ]));
    assert_eq!(
        commit_with_inventory(&mut app, source, ability),
        Err(AbilityCommitError::AdditionalCost(
            AdditionalCostError::UnsupportedResource {
                resource: unsupported
            }
        ))
    );
    assert_eq!(balance(&app, source, bomb), 1);
}

#[test]
fn cancelling_a_successful_activation_does_not_refund_its_items() {
    let mut app = inventory_app();
    let bomb = resource_name(&mut app, "Item.Bomb");
    let source = spawn_inventory_owner(&mut app, bomb, 1);
    let handle = give_ability(&mut app, source, Arc::new(item_ability(bomb, 1)));
    assert_eq!(
        activate_with_inventory(&mut app, source, source, handle),
        Ok(())
    );
    let active = active_ability_entity_for_spec(&mut app, handle).unwrap();
    assert!(
        app.world_mut()
            .run_system_once(move |mut params: AbilitySystemParams<InventoryCosts>| {
                cancel_ability(source, active, &mut params)
            })
            .unwrap()
    );
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(active_ability_count(&mut app), 0);
    assert_eq!(balance(&app, source, bomb), 0);
    assert_eq!(app.world().resource::<ProviderLog>().rollback_calls, 0);
}

#[test]
fn configured_runtime_plugins_share_the_default_runtime_identity() {
    let default = GameplayAbilitySystemRuntimePlugin;
    let configured = GameplayAbilitySystemRuntimePlugin::with_additional_costs::<InventoryCosts>();
    let empty = GameplayAbilitySystemRuntimePlugin::with_additional_costs::<()>();
    assert_eq!(configured.name(), default.name());
    assert_eq!(empty.name(), default.name());
}

#[test]
fn repeated_cost_amount_overflow_fails_without_debiting_inventory() {
    let mut app = inventory_app();
    let bomb = resource_name(&mut app, "Item.Bomb");
    let source = spawn_inventory_owner(&mut app, bomb, u32::MAX);
    let ability = Arc::new(GameplayAbility::default().with_additional_costs(vec![
        AdditionalCost::new(bomb, u32::MAX).unwrap(),
        AdditionalCost::new(bomb, 1).unwrap(),
    ]));
    assert_eq!(
        commit_with_inventory(&mut app, source, ability),
        Err(AbilityCommitError::AdditionalCost(
            AdditionalCostError::ProviderFailure {
                reason: "inventory cost total overflow".into()
            }
        ))
    );
    assert_eq!(balance(&app, source, bomb), u32::MAX);
    assert_eq!(app.world().resource::<ProviderLog>().prepare_calls, 0);
}

#[test]
fn cooldown_preparation_failure_does_not_prepare_or_spend_additional_costs() {
    let mut app = inventory_app();
    let bomb = resource_name(&mut app, "Item.Bomb");
    let cooldown_tag = register_tag(&mut app, "Cooldown.Bomb");
    let source = spawn_inventory_owner(&mut app, bomb, 1);
    app.world_mut()
        .entity_mut(source)
        .remove::<GameplayTagContainer>();
    let cooldown = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![cooldown_tag]),
    ));
    let ability = Arc::new(item_ability(bomb, 1).with_cooldown(cooldown));
    assert_eq!(
        commit_with_inventory(&mut app, source, ability),
        Err(AbilityCommitError::CooldownPreparation(
            GameplayEffectApplicationError::MissingTagContainer { target: source }
        ))
    );
    assert_eq!(balance(&app, source, bomb), 1);
    let log = app.world().resource::<ProviderLog>();
    assert_eq!((log.prepare_calls, log.rollback_calls), (0, 0));
}
