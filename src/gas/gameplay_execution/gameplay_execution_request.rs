use crate::ability_system::AbilityActivationRequest;
use crate::gameplay_effects::GameplayEffectApplicationRequest;

/// One deterministic gameplay mutation request.
#[derive(Clone)]
pub enum GameplayExecutionRequest {
    /// Starts an ability after validating its captured activation context.
    ActivateAbility(AbilityActivationRequest),
    /// Applies a gameplay effect with its captured payload.
    ApplyGameplayEffect(GameplayEffectApplicationRequest),
}

impl From<AbilityActivationRequest> for GameplayExecutionRequest {
    fn from(value: AbilityActivationRequest) -> Self {
        Self::ActivateAbility(value)
    }
}

impl From<GameplayEffectApplicationRequest> for GameplayExecutionRequest {
    fn from(value: GameplayEffectApplicationRequest) -> Self {
        Self::ApplyGameplayEffect(value)
    }
}
