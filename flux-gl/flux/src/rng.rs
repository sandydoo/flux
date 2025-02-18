// A common module to handle anything and everything pseudo-random that happens
// in Flux.
//
// We want to have the option of seeding our RNGs to generate determentistic
// output for testing.

use rand::distr::{Alphanumeric, StandardUniform};
use rand::prelude::*;
use rand_pcg::Pcg32;
use rand_seeder::Seeder;
use std::cell::RefCell;
use std::thread_local;

thread_local!(
    static FLUX_RNG: RefCell<Pcg32> = {
        let rng = Pcg32::from_rng(&mut rand::rng());
        RefCell::new(rng)
    }
);

pub fn init_from_seed(optional_seed: &Option<String>) {
    let seed = optional_seed.as_ref().cloned().unwrap_or_else(|| {
        rand::rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect()
    });

    FLUX_RNG.with(|rng| rng.replace(Seeder::from(seed).into_rng()));
}

pub fn gen<T>() -> T
where
    StandardUniform: Distribution<T>,
{
    FLUX_RNG.with(|rng| rng.borrow_mut().random::<T>())
}
