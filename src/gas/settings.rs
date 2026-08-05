//! Compile-time capacity and runtime safety limits for the GAS implementation.

/// Compile-time settings shared across gameplay subsystems.
pub struct GameplayAbilitySystemSettings;

impl GameplayAbilitySystemSettings {
    /// Total number of attribute slots available to an [`crate::AttributeSet`].
    pub const ATTRIBUTE_SET_SIZE: usize = 256;
    /// Number of inline hot attribute slots.
    pub const HOT_ATTRIBUTE_SET_SIZE: usize = 32;
    /// Number of boxed cold attribute slots.
    pub const COLD_ATTRIBUTE_SET_SIZE: usize =
        Self::ATTRIBUTE_SET_SIZE - Self::HOT_ATTRIBUTE_SET_SIZE;
    /// Maximum number of registered gameplay tag bits.
    pub const GAMEPLAY_TAG_SIZE: usize = 512;
    /// Maximum number of nested ability activations in one chain.
    pub const ABILITY_CHAIN_MAX_DEPTH: u8 = 8;
}
