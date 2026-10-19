use bevy::prelude::Resource;
use rand::{
    RngExt, SeedableRng,
    distr::uniform::{SampleRange, SampleUniform},
    rngs::StdRng,
};

/// Seedable random stream used by GAS effect probability evaluation.
///
/// Reproducibility requires the same dependency versions, seed, and sampling order. Mutable
/// resource access prevents simultaneous draws but does not order systems; order callers
/// explicitly, including FIFO producers. Game-owned spatial and presentation sampling should
/// use separate resources so their draws do not change effect probability outcomes.
#[derive(Resource, Debug)]
pub struct Random {
    rng: StdRng,
}

impl Default for Random {
    fn default() -> Self {
        Self::from_seed(Self::DEFAULT_SEED)
    }
}

impl Random {
    /// Seed used when the resource is initialized with its default configuration.
    pub const DEFAULT_SEED: u64 = 123456;

    /// Creates a fresh stream initialized with `seed`.
    pub fn from_seed(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }

    /// Restarts this stream at `seed`, discarding its previous sampling position.
    pub fn set_seed(&mut self, seed: u64) {
        self.rng = StdRng::seed_from_u64(seed);
    }

    /// Samples a value from a valid, non-empty `range`, advancing this shared stream.
    /// Reproducible results require the same sampling order across all callers.
    pub fn random_range<T, R>(&mut self, range: R) -> T
    where
        T: SampleUniform,
        R: SampleRange<T>,
    {
        self.rng.random_range(range)
    }

    /// Samples a boolean with the supplied probability of returning `true`.
    /// `probability` must be finite and within `0.0..=1.0`. Reproducible results require the
    /// same sampling order across all callers of this shared stream.
    pub fn random_bool(&mut self, probability: f32) -> bool {
        self.rng.random_bool(f64::from(probability))
    }
}
