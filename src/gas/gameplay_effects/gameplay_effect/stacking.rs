#[derive(Debug, Clone, Copy)]
pub enum StackingType {
    None,
    AggregateBySource,
    AggregateByTarget,
}

#[derive(Debug, Clone, Copy)]
pub enum StackMagnitudePolicy {
    None,
    Linear,
}

#[derive(Debug, Clone, Copy)]
pub enum StackDurationPolicy {
    KeepExisting,
    RefreshOnSuccessfulStack,
}

#[derive(Debug, Clone, Copy)]
pub enum StackPeriodPolicy {
    KeepCurrentTick,
    ResetOnSuccessfulStack,
}

#[derive(Debug, Clone, Copy)]
pub enum StackOverflowPolicy {
    RejectApplication,
    RefreshDuration,
}

#[derive(Debug, Clone, Copy)]
pub enum StackExpirationPolicy {
    RemoveAllStacks,
    RemoveSingleStack,
}

#[derive(Debug, Clone, Copy)]
pub struct StackingPolicy {
    stacking_type: StackingType,
    stack_limit: u32,
    magnitude_policy: StackMagnitudePolicy,
    duration_policy: StackDurationPolicy,
    period_policy: StackPeriodPolicy,
    overflow_policy: StackOverflowPolicy,
    expiration_policy: StackExpirationPolicy,
}

impl StackingPolicy {
    pub fn new(
        stacking_type: StackingType,
        stack_limit: u32,
        magnitude_policy: StackMagnitudePolicy,
        duration_policy: StackDurationPolicy,
        period_policy: StackPeriodPolicy,
        overflow_policy: StackOverflowPolicy,
        expiration_policy: StackExpirationPolicy,
    ) -> Self {
        Self {
            stacking_type,
            stack_limit,
            magnitude_policy,
            duration_policy,
            period_policy,
            overflow_policy,
            expiration_policy,
        }
    }

    pub fn non_stacking() -> Self {
        Self::new(
            StackingType::None,
            0,
            StackMagnitudePolicy::None,
            StackDurationPolicy::KeepExisting,
            StackPeriodPolicy::KeepCurrentTick,
            StackOverflowPolicy::RejectApplication,
            StackExpirationPolicy::RemoveAllStacks,
        )
    }

    pub fn linear_refreshing(stacking_type: StackingType, stack_limit: u32) -> Self {
        Self::new(
            stacking_type,
            stack_limit,
            StackMagnitudePolicy::Linear,
            StackDurationPolicy::RefreshOnSuccessfulStack,
            StackPeriodPolicy::ResetOnSuccessfulStack,
            StackOverflowPolicy::RejectApplication,
            StackExpirationPolicy::RemoveAllStacks,
        )
    }

    pub fn get_stacking_type(&self) -> StackingType {
        self.stacking_type
    }

    pub fn get_stack_limit(&self) -> u32 {
        self.stack_limit
    }

    pub fn get_magnitude_policy(&self) -> StackMagnitudePolicy {
        self.magnitude_policy
    }

    pub fn get_duration_policy(&self) -> StackDurationPolicy {
        self.duration_policy
    }

    pub fn get_period_policy(&self) -> StackPeriodPolicy {
        self.period_policy
    }

    pub fn get_overflow_policy(&self) -> StackOverflowPolicy {
        self.overflow_policy
    }

    pub fn get_expiration_policy(&self) -> StackExpirationPolicy {
        self.expiration_policy
    }
}
