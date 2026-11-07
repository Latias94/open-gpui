use std::env;

/// Returns the seeds a randomized `#[open_gpui::test]` or `#[open_gpui::bench]` runs with, in
/// order, and whether there is more than one.
///
/// With `$SEED` set, runs `iterations` consecutive seeds starting at `$SEED` and ignores
/// `explicit_seeds`. A sequence whose final seed exceeds `u64::MAX` is rejected before iteration.
/// Otherwise runs `0..iterations` followed by `explicit_seeds`, except that the
/// default single iteration is dropped when explicit seeds are given.
/// `$ITERATIONS` overrides `iterations`.
/// Tests and benchmarks share this so that one `SEED` reproduces either.
///
/// This is intended for use by the `open_gpui::test` and `open_gpui::bench` macros and generally
/// should not be used directly.
#[doc(hidden)]
pub fn calculate_seeds(
    iterations: u64,
    explicit_seeds: &[u64],
) -> (impl Iterator<Item = u64> + '_, bool) {
    let iterations = env::var("ITERATIONS")
        .ok()
        .map(|var| var.parse().expect("invalid ITERATIONS variable"))
        .unwrap_or(iterations);

    let env_seed = env::var("SEED")
        .map(|seed| seed.parse().expect("invalid SEED variable as integer"))
        .ok();

    let iter = seeds(iterations, explicit_seeds, env_seed);
    let is_multiple_runs = iter.clone().nth(1).is_some();
    (iter, is_multiple_runs)
}

fn seeds(
    iterations: u64,
    explicit_seeds: &[u64],
    env_seed: Option<u64>,
) -> impl Iterator<Item = u64> + Clone + '_ {
    let (iterations_range, offset, explicit_seeds) = match env_seed {
        Some(env_seed) => {
            if iterations > 0 {
                env_seed
                    .checked_add(iterations - 1)
                    .expect("SEED and ITERATIONS exceed the u64 seed range");
            }
            (0..iterations, env_seed, &[][..])
        }
        None if iterations == 1 && !explicit_seeds.is_empty() => (0..0, 0, explicit_seeds),
        None => (0..iterations, 0, explicit_seeds),
    };
    iterations_range
        .map(move |index| offset + index)
        .chain(explicit_seeds.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::{calculate_seeds, seeds};

    fn collect(iterations: u64, explicit_seeds: &[u64], env_seed: Option<u64>) -> Vec<u64> {
        seeds(iterations, explicit_seeds, env_seed).collect()
    }

    #[test]
    fn seeds_follow_iterations_explicit_seeds_and_seed_variable() {
        assert_eq!(collect(1, &[], None), [0]);
        assert_eq!(collect(3, &[], None), [0, 1, 2]);
        assert_eq!(collect(1, &[10, 20], None), [10, 20]);
        assert_eq!(collect(2, &[10], None), [0, 1, 10]);
        assert_eq!(collect(1, &[10], Some(7)), [7]);
        assert_eq!(
            collect(2, &[10], Some(7)),
            [7, 8],
            "`SEED` starts the iterations once, without repeating itself"
        );
    }

    #[test]
    fn seeds_keep_explicit_order_and_repetitions() {
        assert_eq!(collect(1, &[20, 10, 20], None), [20, 10, 20]);
        assert_eq!(collect(2, &[1, 0], None), [0, 1, 1, 0]);
    }

    #[test]
    fn zero_iterations_obeys_seed_override() {
        assert!(collect(0, &[], None).is_empty());
        assert_eq!(collect(0, &[10], None), [10]);
        assert!(collect(0, &[10], Some(7)).is_empty());
    }

    #[test]
    fn maximum_seed_runs_once_without_an_exclusive_upper_bound() {
        assert_eq!(collect(1, &[], Some(u64::MAX)), [u64::MAX]);
        assert_eq!(
            collect(2, &[], Some(u64::MAX - 1)),
            [u64::MAX - 1, u64::MAX]
        );
        assert!(collect(0, &[], Some(u64::MAX)).is_empty());
    }

    #[test]
    #[should_panic(expected = "SEED and ITERATIONS exceed the u64 seed range")]
    fn overflowing_seed_sequence_is_rejected_before_iteration() {
        let _ = seeds(2, &[], Some(u64::MAX));
    }

    #[test]
    fn calculate_seeds_env_override() {
        const CHILD_ENV: &str = "OPEN_GPUI_SEEDS_ENV_OVERRIDE_TEST_CHILD";
        if std::env::var_os(CHILD_ENV).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "seeds::tests::calculate_seeds_env_override",
                    "--nocapture",
                ])
                .env(CHILD_ENV, "1")
                .env("SEED", "7")
                .env("ITERATIONS", "2")
                .output()
                .expect("seed override regression subprocess must start");
            assert!(
                output.status.success(),
                "seed override regression failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let (seeds, is_multiple_runs) = calculate_seeds(5, &[10, 20]);
        assert_eq!(seeds.collect::<Vec<_>>(), [7, 8]);
        assert!(is_multiple_runs);
    }
}
