// A common module to handle anything and everything psuedo-random that happens
// in Flux.
//
// We want to have the option of seeding our RNGs to generate determentistic
// output for testing.

use rand::distributions::Alphanumeric;
use rand::prelude::*;
use rand_pcg::Pcg32;
use rand_seeder::Seeder;

pub use rand::Rng;
pub type FRng = Pcg32;

pub fn from_seed(optional_seed: &Option<String>) -> FRng {
    let seed = optional_seed.as_ref().cloned().unwrap_or_else(|| {
        thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect()
    });

    Seeder::from(seed).make_rng()
}
