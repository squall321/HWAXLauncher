//! Retry/poll delay policy for the agent's periodic loops.
//!
//! Two goals, both sharpened now that the hub rate-limits launcher traffic
//! (per-agent budgets on `/launcher-agents/*` and `/installers/*`):
//!   - **De-synchronize the fleet** — always add a little jitter so a thousand
//!     agents behind one egress don't poll in lockstep (thundering herd against
//!     a recovering server).
//!   - **Back off on repeated failure** — when the server is down or returning
//!     429, wait progressively longer instead of polling on a fixed cadence.
//!
//! Pure + deterministic (jitter is seeded by the caller, e.g. from the system
//! clock) so it is unit-testable.

/// Next loop delay, in seconds: `base + failure penalty + jitter`.
///
/// `base_secs` is the loop's normal cadence. Each consecutive failure adds 30 s
/// (capped at 10 failures → +300 s) so a flapping/rate-limited server is polled
/// ever more gently. Jitter adds up to +12.5 % from `jitter_seed`.
pub fn next_delay_secs(base_secs: u64, consecutive_failures: u32, jitter_seed: u64) -> u64 {
    let penalty = u64::from(consecutive_failures.min(10)) * 30;
    let target = base_secs.saturating_add(penalty);
    with_jitter(target, jitter_seed)
}

/// Add `0..=secs/8` (≈ up to +12.5 %) of deterministic jitter from `seed`.
pub fn with_jitter(secs: u64, seed: u64) -> u64 {
    let span = secs / 8 + 1;
    secs.saturating_add(seed % span)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_failure_is_base_plus_bounded_jitter() {
        for seed in [0u64, 1, 7, 999, u64::MAX] {
            let d = next_delay_secs(1800, 0, seed);
            assert!(d >= 1800, "never shorter than base");
            assert!(d <= 1800 + 1800 / 8, "jitter is bounded to +12.5%");
        }
    }

    #[test]
    fn failures_back_off_monotonically_capped() {
        let seed = 0; // no jitter at seed 0
        assert_eq!(next_delay_secs(30, 0, seed), 30);
        assert_eq!(next_delay_secs(30, 1, seed), 60);
        assert_eq!(next_delay_secs(30, 3, seed), 120);
        // capped at 10 failures (+300s) — more failures don't grow further
        assert_eq!(next_delay_secs(30, 10, seed), 330);
        assert_eq!(next_delay_secs(30, 50, seed), 330);
    }
}
