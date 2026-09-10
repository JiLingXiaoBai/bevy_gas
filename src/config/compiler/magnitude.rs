pub(crate) fn evaluate_linear(base: f32, per_level: f32, level: u32) -> f64 {
    f64::from(base) + f64::from(per_level) * f64::from(level.saturating_sub(1))
}
