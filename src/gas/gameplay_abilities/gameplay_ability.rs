use super::AbilityTaskDef;
use crate::gameplay_effects::GameplayEffect;
use crate::gameplay_tags::GameplayTag;
use std::sync::Arc;

/// Configures an ability's identity and tag-based activation rules.
///
/// Asset tags identify the ability; they are not granted to its owner. Cancel and block
/// rules match other abilities' asset tags. Activation requirements inspect the owner's
/// gameplay tags. The default configuration contains no tags or requirements.
#[derive(Default)]
pub struct AbilityTags {
    ability_asset_tags: Vec<GameplayTag>,
    cancel_abilities_with_tags: Vec<GameplayTag>,
    block_abilities_with_tags: Vec<GameplayTag>,
    activation_required_tags: Vec<GameplayTag>,
    activation_blocked_tags: Vec<GameplayTag>,
}

impl AbilityTags {
    /// Creates a tag configuration from all five rule groups.
    ///
    /// `ability_asset_tags` identify this ability. `cancel_abilities_with_tags` select
    /// active abilities to cancel at startup, while `block_abilities_with_tags` select
    /// abilities blocked while this ability is active. `activation_required_tags` must
    /// all be present on the owner, and any `activation_blocked_tags` prevent activation.
    /// Returns the configuration without registering or granting any tags.
    ///
    /// Use [`Self::default`] and the `with_*` methods to configure individual groups by name.
    pub fn new(
        ability_asset_tags: Vec<GameplayTag>,
        cancel_abilities_with_tags: Vec<GameplayTag>,
        block_abilities_with_tags: Vec<GameplayTag>,
        activation_required_tags: Vec<GameplayTag>,
        activation_blocked_tags: Vec<GameplayTag>,
    ) -> Self {
        Self {
            ability_asset_tags,
            cancel_abilities_with_tags,
            block_abilities_with_tags,
            activation_required_tags,
            activation_blocked_tags,
        }
    }

    /// Replaces this ability's identifying asset tags with `tags` and returns the configuration.
    pub fn with_ability_asset_tags(mut self, tags: Vec<GameplayTag>) -> Self {
        self.ability_asset_tags = tags;
        self
    }

    /// Replaces the asset tags used to cancel other active abilities at startup.
    ///
    /// Any matching tag in `tags` selects an ability for cancellation. Returns the configuration.
    pub fn with_cancel_abilities_with_tags(mut self, tags: Vec<GameplayTag>) -> Self {
        self.cancel_abilities_with_tags = tags;
        self
    }

    /// Replaces the asset tags used to block other abilities while this ability is active.
    ///
    /// Any matching tag in `tags` blocks activation. Returns the configuration.
    pub fn with_block_abilities_with_tags(mut self, tags: Vec<GameplayTag>) -> Self {
        self.block_abilities_with_tags = tags;
        self
    }

    /// Replaces the owner tags that must all be present for activation with `tags`.
    ///
    /// Returns the updated configuration.
    pub fn with_activation_required_tags(mut self, tags: Vec<GameplayTag>) -> Self {
        self.activation_required_tags = tags;
        self
    }

    /// Replaces the owner tags that prevent activation when any are present with `tags`.
    ///
    /// Returns the updated configuration.
    pub fn with_activation_blocked_tags(mut self, tags: Vec<GameplayTag>) -> Self {
        self.activation_blocked_tags = tags;
        self
    }

    /// Returns the asset tags identifying this ability.
    pub fn get_ability_asset_tags(&self) -> &[GameplayTag] {
        &self.ability_asset_tags
    }

    /// Returns the asset tags used to cancel matching active abilities at startup.
    pub fn get_cancel_abilities_with_tags(&self) -> &[GameplayTag] {
        &self.cancel_abilities_with_tags
    }

    /// Returns the asset tags blocked while this ability is active.
    pub fn get_block_abilities_with_tags(&self) -> &[GameplayTag] {
        &self.block_abilities_with_tags
    }

    /// Returns the owner tags that must all be present for activation.
    pub fn get_activation_required_tags(&self) -> &[GameplayTag] {
        &self.activation_required_tags
    }

    /// Returns the owner tags that prevent activation when any are present.
    pub fn get_activation_blocked_tags(&self) -> &[GameplayTag] {
        &self.activation_blocked_tags
    }
}

/// Defines shared ability rules, cost and cooldown effects, and startup tasks.
///
/// Definitions are shared through [`Arc`]; each activation has its own runtime state.
/// Activation automatically commits cost and cooldown effects to the owner before starting
/// tasks. Callers do not need to commit again after activation succeeds. Use Instant startup
/// tasks to apply effects during activation and an explicit EndAbility action to end the instance.
///
/// Startup tasks run in definition order. Waiting tasks count from activation rather than
/// waiting for the previous task to finish. Completing startup leaves the ability active unless
/// an action explicitly ends or cancels it. Ending the ability cleans up its tasks but does not
/// remove already applied effects; those effects follow their own lifetimes.
///
/// The default definition has empty tags and tasks, no cost or cooldown, and disallows multiple
/// active instances. It remains active until explicitly ended or cancelled.
#[derive(Default)]
pub struct GameplayAbility {
    ability_tags: AbilityTags,
    startup_tasks: Vec<AbilityTaskDef>,
    cooldown: Option<Arc<GameplayEffect>>,
    cost: Option<Arc<GameplayEffect>>,
    allow_multiple_instances: bool,
}

impl GameplayAbility {
    /// Creates an ability definition with all configuration supplied explicitly.
    ///
    /// `ability_tags` control identity and activation rules. `startup_tasks` start in definition
    /// order during activation; Instant effects and chained activations resolve inline, while
    /// waiting tasks run concurrently. Event observers remain deferred.
    /// `cooldown` and `cost` are committed to the owner automatically. `allow_multiple_instances`
    /// permits concurrent activations of the same granted ability. Completing startup does not
    /// end the instance; use an explicit EndAbility action or end or cancel it through the ability
    /// system. Returns the definition without granting or activating it.
    ///
    /// Use [`Self::default`] and the `with_*` methods to configure individual fields by name.
    pub fn new(
        ability_tags: AbilityTags,
        startup_tasks: Vec<AbilityTaskDef>,
        cooldown: Option<Arc<GameplayEffect>>,
        cost: Option<Arc<GameplayEffect>>,
        allow_multiple_instances: bool,
    ) -> Self {
        Self {
            ability_tags,
            startup_tasks,
            cooldown,
            cost,
            allow_multiple_instances,
        }
    }

    /// Replaces the ability's tag configuration with `tags` and returns the definition.
    pub fn with_tags(mut self, tags: AbilityTags) -> Self {
        self.ability_tags = tags;
        self
    }

    /// Replaces the startup tasks with `tasks` and returns the definition.
    ///
    /// Tasks start in definition order, but waiting tasks count from activation concurrently.
    /// Instant effects and chained activations finish before the next startup action begins.
    /// Child cancellation stops the parent's remaining startup actions. Event observers stay deferred.
    /// An instant task that ends the ability prevents subsequent startup tasks from starting.
    /// Completing startup without an explicit end leaves the ability active.
    pub fn with_startup_tasks(mut self, tasks: Vec<AbilityTaskDef>) -> Self {
        self.startup_tasks = tasks;
        self
    }

    /// Sets `cooldown` as the effect automatically committed to the owner at activation.
    ///
    /// Its granted tags block later activations while present on the owner. The effect
    /// follows its own lifetime after the ability ends. Returns the definition.
    pub fn with_cooldown(mut self, cooldown: Arc<GameplayEffect>) -> Self {
        self.cooldown = Some(cooldown);
        self
    }

    /// Sets `cost` as the effect automatically committed to the owner at activation.
    ///
    /// Costs must be instant effects containing only additive modifiers. Negative values
    /// consume the corresponding attributes after affordability checks. Returns the definition.
    pub fn with_cost(mut self, cost: Arc<GameplayEffect>) -> Self {
        self.cost = Some(cost);
        self
    }

    /// Sets whether the same granted ability may have concurrent active instances.
    ///
    /// `allow_multiple_instances` does not bypass cost, cooldown, or tag requirements.
    /// Returns the updated definition.
    pub fn with_allow_multiple_instances(mut self, allow_multiple_instances: bool) -> Self {
        self.allow_multiple_instances = allow_multiple_instances;
        self
    }

    /// Returns the ability's identity and activation tag rules.
    pub fn get_tags(&self) -> &AbilityTags {
        &self.ability_tags
    }

    /// Returns the tasks started during activation, in definition order.
    pub fn get_startup_tasks(&self) -> &[AbilityTaskDef] {
        &self.startup_tasks
    }

    /// Returns the cooldown effect automatically committed to the owner, if configured.
    pub fn get_cooldown(&self) -> Option<&Arc<GameplayEffect>> {
        self.cooldown.as_ref()
    }

    /// Returns the cost effect automatically committed to the owner, if configured.
    pub fn get_cost(&self) -> Option<&Arc<GameplayEffect>> {
        self.cost.as_ref()
    }

    /// Returns whether the same granted ability may have concurrent active instances.
    pub fn allow_multiple_instances(&self) -> bool {
        self.allow_multiple_instances
    }
}
