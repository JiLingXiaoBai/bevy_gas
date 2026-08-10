use super::{
    AbilityActivationStatus, AbilityTask, AbilityTaskCompletion, ActiveGameplayAbility,
    dispatch_ability_task_completion,
};
use crate::gameplay_execution::GameplayExecutionQueue;
use bevy::prelude::*;

pub fn tick_ability_tasks_system(
    mut commands: Commands,
    mut task_query: Query<(Entity, &mut AbilityTask)>,
    mut active_ability_query: Query<&mut ActiveGameplayAbility>,
    mut execution_queue: ResMut<GameplayExecutionQueue>,
) {
    let mut task_entities: Vec<_> = task_query
        .iter_mut()
        .map(|(task_entity, _)| task_entity)
        .collect();
    task_entities.sort_by_key(|entity| entity.to_bits());

    for task_entity in task_entities {
        let (active_handle, task_context, active_context, on_finished) = {
            let Ok((_, mut task)) = task_query.get_mut(task_entity) else {
                continue;
            };
            let Ok(active_ability) = active_ability_query.get(task.get_active_ability()) else {
                commands.entity(task_entity).despawn();
                continue;
            };

            let active_status = active_ability.get_status();
            let active_context = active_ability.get_activation_context().clone();

            if !matches!(active_status, AbilityActivationStatus::Active) {
                commands.entity(task_entity).despawn();
                continue;
            }

            if !task.tick() {
                continue;
            }

            (
                task.get_active_ability(),
                *task.get_context(),
                active_context,
                task.get_on_finished().clone(),
            )
        };

        if matches!(
            dispatch_ability_task_completion(
                active_handle,
                task_context,
                on_finished,
                &active_context,
                &mut commands,
                &mut execution_queue,
            ),
            AbilityTaskCompletion::EndAbility
        ) && let Ok(mut active_ability) = active_ability_query.get_mut(active_handle)
        {
            active_ability.set_status(AbilityActivationStatus::Ending);
        }

        commands.entity(task_entity).despawn();
    }
}
