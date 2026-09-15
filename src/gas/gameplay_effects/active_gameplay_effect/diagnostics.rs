//! Optional observations of requirement convergence work.

use bevy::prelude::Resource;
use std::time::Duration;

/// Accumulated observations from measured requirement-convergence calls.
#[derive(Debug, Clone, Copy, Default)]
pub struct EffectRequirementMetrics {
    /// Number of completed convergence calls.
    pub convergence_calls: u64,
    /// Number of requirement decision passes, including the final unchanged pass.
    pub decision_passes: u64,
    /// Active effects visited while building cycle-detection signatures.
    pub signature_effect_visits: u64,
    /// Active effects visited while deciding requirement transitions.
    pub decision_effect_visits: u64,
    /// Complete effect snapshots copied for requirement decisions.
    pub effect_snapshots: u64,
    /// Requirement transitions applied across all passes.
    pub transitions: u64,
    /// Non-converging cycles detected and removed.
    pub cycles: u64,
    /// Total time spent inside measured convergence calls.
    pub elapsed: Duration,
}

/// Optional requirement-convergence measurements, disabled by default.
///
/// Register this resource and enable it to accumulate work counts and elapsed time.
/// Timing is observational only and never controls gameplay decisions.
#[derive(Resource, Debug, Default)]
pub struct EffectRequirementDiagnostics {
    enabled: bool,
    metrics: EffectRequirementMetrics,
}

impl EffectRequirementDiagnostics {
    /// Enables or disables recording without changing the accumulated measurements.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Returns whether subsequent convergence calls record measurements.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the accumulated measurements.
    pub fn metrics(&self) -> &EffectRequirementMetrics {
        &self.metrics
    }

    /// Clears all accumulated measurements without changing the enabled state.
    pub fn reset(&mut self) {
        self.metrics = EffectRequirementMetrics::default();
    }

    pub(super) fn record(&mut self, sample: EffectRequirementMetrics) {
        self.metrics.convergence_calls = self
            .metrics
            .convergence_calls
            .saturating_add(sample.convergence_calls);
        self.metrics.decision_passes = self
            .metrics
            .decision_passes
            .saturating_add(sample.decision_passes);
        self.metrics.signature_effect_visits = self
            .metrics
            .signature_effect_visits
            .saturating_add(sample.signature_effect_visits);
        self.metrics.decision_effect_visits = self
            .metrics
            .decision_effect_visits
            .saturating_add(sample.decision_effect_visits);
        self.metrics.effect_snapshots = self
            .metrics
            .effect_snapshots
            .saturating_add(sample.effect_snapshots);
        self.metrics.transitions = self.metrics.transitions.saturating_add(sample.transitions);
        self.metrics.cycles = self.metrics.cycles.saturating_add(sample.cycles);
        self.metrics.elapsed = self.metrics.elapsed.saturating_add(sample.elapsed);
    }
}
