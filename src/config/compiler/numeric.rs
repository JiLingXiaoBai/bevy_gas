//! Pure numeric constraints shared by construction and optional authoring validation.

pub(super) fn probability_is_valid(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

pub(super) fn formula_parameters_are_finite(base: f32, per_level: Option<f32>) -> bool {
    base.is_finite() && per_level.is_none_or(f32::is_finite)
}

pub(super) fn square_is_finite(value: f32) -> bool {
    (value * value).is_finite()
}

pub(super) fn is_within_f32_range(value: f64) -> bool {
    value.is_finite() && value.abs() <= f64::from(f32::MAX)
}
