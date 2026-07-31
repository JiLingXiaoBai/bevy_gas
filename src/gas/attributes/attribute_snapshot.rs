#[derive(Debug, Clone, Copy)]
pub struct AttributeSnapshot {
    base: f32,
    current: f32,
}

impl AttributeSnapshot {
    pub fn new(base: f32, current: f32) -> Self {
        Self { base, current }
    }

    pub fn base(&self) -> f32 {
        self.base
    }

    pub fn current(&self) -> f32 {
        self.current
    }
}
