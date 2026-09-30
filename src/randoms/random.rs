use bevy::math::{FromRng, ShapeSample};
use bevy::prelude::*;
use rand::{
    RngExt, SeedableRng,
    distr::{Distribution, StandardUniform, uniform::SampleRange, uniform::SampleUniform},
    rngs::StdRng,
};

/// Seedable random stream shared by gameplay systems in one World.
///
/// Reproducibility requires the same dependency versions, seed, and sampling order. Mutable
/// resource access prevents simultaneous draws but does not order systems; order callers
/// explicitly, including FIFO producers, or use separate seeded streams for independent work.
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

    /// Samples a value of `T` using Bevy's [`FromRng`] implementation and this stream.
    pub fn from_rng<T>(&mut self) -> T
    where
        T: FromRng,
        StandardUniform: Distribution<T>,
    {
        T::from_rng(&mut self.rng)
    }

    /// Samples a point inside `shape` using this stream.
    pub fn sample_interior<S>(&mut self, shape: &S) -> S::Output
    where
        S: ShapeSample,
    {
        shape.sample_interior(&mut self.rng)
    }

    /// Samples a point on the boundary of `shape` using this stream.
    pub fn sample_boundary<S>(&mut self, shape: &S) -> S::Output
    where
        S: ShapeSample,
    {
        shape.sample_boundary(&mut self.rng)
    }
}
