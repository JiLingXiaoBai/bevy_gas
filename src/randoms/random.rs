use bevy::math::{FromRng, ShapeSample};
use bevy::prelude::*;
use rand::{
    RngExt, SeedableRng,
    distr::{Distribution, StandardUniform, uniform::SampleRange, uniform::SampleUniform},
    rngs::StdRng,
};

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
    pub const DEFAULT_SEED: u64 = 123456;

    pub fn from_seed(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }

    pub fn set_seed(&mut self, seed: u64) {
        self.rng = StdRng::seed_from_u64(seed);
    }

    pub fn random_range<T, R>(&mut self, range: R) -> T
    where
        T: SampleUniform,
        R: SampleRange<T>,
    {
        self.rng.random_range(range)
    }

    pub fn random_bool(&mut self, probability: f32) -> bool {
        self.rng.random_bool(f64::from(probability))
    }

    pub fn from_rng<T>(&mut self) -> T
    where
        T: FromRng,
        StandardUniform: Distribution<T>,
    {
        T::from_rng(&mut self.rng)
    }

    pub fn sample_interior<S>(&mut self, shape: &S) -> S::Output
    where
        S: ShapeSample,
    {
        shape.sample_interior(&mut self.rng)
    }

    pub fn sample_boundary<S>(&mut self, shape: &S) -> S::Output
    where
        S: ShapeSample,
    {
        shape.sample_boundary(&mut self.rng)
    }
}
