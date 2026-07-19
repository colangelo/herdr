//! Reusable settle-and-bubble motion for reordering lists.
//!
//! A `ListMotion` owns the *display* order of a list whose *target* order is
//! recomputed elsewhere (e.g. priority sort). Instead of teleporting to the
//! target on the next frame, a diverged entry holds its position for a settle
//! delay, then bubbles one position per step interval until it reaches its
//! slot — up or down alike. New keys appear at their target position
//! immediately and removed keys disappear immediately; only reorders of
//! existing keys are animated.
//!
//! The display order mutates only in [`ListMotion::tick`]. Read paths use
//! [`ListMotion::project`], which is pure, so rendering, hit-testing, and any
//! other consumer observe one coherent order between ticks.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ListMotionTiming {
    /// How long a diverged entry holds its position before it starts moving.
    pub settle: Duration,
    /// Interval between one-position steps once an entry is moving.
    pub step: Duration,
}

/// Returns when the next step after `now` is due. Constant interval for now;
/// this is the single seam where an easing curve (slow start, accelerate,
/// decelerate into the slot — like a bubble in nature) can replace the timing
/// without touching consumers.
fn next_step_delay(timing: ListMotionTiming) -> Duration {
    timing.step
}

#[derive(Debug, Default)]
pub(crate) struct ListMotion<K> {
    display: Vec<K>,
    /// When each currently-diverged key first left its target position; a key
    /// may move only after holding for the settle delay. Cleared on alignment.
    diverged_since: HashMap<K, Instant>,
    /// Step cadence: one adjacent swap per interval while motion is running.
    next_step_at: Option<Instant>,
}

impl<K: Eq + Hash + Clone> ListMotion<K> {
    pub(crate) fn new() -> Self {
        Self {
            display: Vec::new(),
            diverged_since: HashMap::new(),
            next_step_at: None,
        }
    }

    /// Pure projection of `target` through the current display order:
    /// keys already displayed keep their displayed relative order, keys new to
    /// `target` are inserted at their target position, and keys absent from
    /// `target` are dropped. Does not mutate; safe for render and hit-testing.
    pub(crate) fn project(&self, target: &[K]) -> Vec<K> {
        let mut result: Vec<K> = self
            .display
            .iter()
            .filter(|key| target.contains(key))
            .cloned()
            .collect();
        for (target_idx, key) in target.iter().enumerate() {
            if !result.contains(key) {
                let at = target_idx.min(result.len());
                result.insert(at, key.clone());
            }
        }
        result
    }

    /// Advances motion at `now` toward `target` and returns the new display
    /// order. The only mutation point.
    ///
    /// At most one adjacent swap happens per step interval, chosen as the
    /// first out-of-order neighbor pair (by target position) whose keys have
    /// both held their diverged position for the settle delay — i.e. one
    /// bubble-sort step per tick, so a travelling row visibly moves one slot
    /// at a time and the list never teleports.
    pub(crate) fn tick(&mut self, now: Instant, target: &[K], timing: ListMotionTiming) -> &[K] {
        self.display = self.project(target);
        self.refresh_divergence(now, target);
        if self.diverged_since.is_empty() {
            self.next_step_at = None;
            return &self.display;
        }
        if self.next_step_at.is_some_and(|at| now < at) {
            return &self.display;
        }

        let target_pos =
            |key: &K| -> Option<usize> { target.iter().position(|other| other == key) };
        let released = |diverged_since: &HashMap<K, Instant>, key: &K| {
            diverged_since
                .get(key)
                .is_some_and(|since| now >= *since + timing.settle)
        };
        for idx in 0..self.display.len().saturating_sub(1) {
            let inverted = match (
                target_pos(&self.display[idx]),
                target_pos(&self.display[idx + 1]),
            ) {
                (Some(upper), Some(lower)) => upper > lower,
                _ => false,
            };
            if !inverted
                || !released(&self.diverged_since, &self.display[idx])
                || !released(&self.diverged_since, &self.display[idx + 1])
            {
                continue;
            }
            self.display.swap(idx, idx + 1);
            self.next_step_at = Some(now + next_step_delay(timing));
            self.refresh_divergence(now, target);
            break;
        }

        &self.display
    }

    /// Rebuilds the diverged-key set against `target`: aligned keys forget
    /// their divergence, newly diverged keys start their settle clock at `now`.
    fn refresh_divergence(&mut self, now: Instant, target: &[K]) {
        for (display_idx, key) in self.display.iter().enumerate() {
            if target.iter().position(|other| other == key) == Some(display_idx) {
                self.diverged_since.remove(key);
            } else {
                self.diverged_since.entry(key.clone()).or_insert(now);
            }
        }
        let display = &self.display;
        self.diverged_since.retain(|key, _| display.contains(key));
    }

    /// Earliest instant at which [`tick`](Self::tick) has work to do, for the
    /// loop-deadline aggregator. `None` when the display order is settled.
    pub(crate) fn next_due(&self, timing: ListMotionTiming) -> Option<Instant> {
        let earliest_release = self
            .diverged_since
            .values()
            .map(|since| *since + timing.settle)
            .min()?;
        Some(match self.next_step_at {
            Some(step_at) => step_at.max(earliest_release),
            None => earliest_release,
        })
    }

    /// Forgets all motion state; the next tick snaps to the target order.
    /// Used when motion is disabled or the list leaves priority sort.
    pub(crate) fn reset(&mut self) {
        self.display.clear();
        self.diverged_since.clear();
        self.next_step_at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMING: ListMotionTiming = ListMotionTiming {
        settle: Duration::from_millis(2000),
        step: Duration::from_millis(150),
    };

    fn keys(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn tick(motion: &mut ListMotion<String>, now: Instant, target: &[&str]) -> Vec<String> {
        motion.tick(now, &keys(target), TIMING).to_vec()
    }

    #[test]
    fn stable_target_stays_put() {
        let mut motion = ListMotion::new();
        let t0 = Instant::now();
        assert_eq!(
            tick(&mut motion, t0, &["a", "b", "c"]),
            keys(&["a", "b", "c"])
        );
        assert_eq!(motion.next_due(TIMING), None);
        assert_eq!(
            tick(&mut motion, t0 + TIMING.step, &["a", "b", "c"]),
            keys(&["a", "b", "c"])
        );
    }

    #[test]
    fn divergence_holds_through_settle_then_steps() {
        let mut motion = ListMotion::new();
        let t0 = Instant::now();
        tick(&mut motion, t0, &["a", "b", "c"]);

        // "a" drops to the bottom (e.g. done -> seen).
        let target = ["b", "c", "a"];
        assert_eq!(tick(&mut motion, t0, &target), keys(&["a", "b", "c"]));
        assert_eq!(motion.next_due(TIMING), Some(t0 + TIMING.settle));

        // Still held just before settle expiry.
        let almost = t0 + TIMING.settle - Duration::from_millis(1);
        assert_eq!(tick(&mut motion, almost, &target), keys(&["a", "b", "c"]));

        // First step at settle expiry: one position only.
        let settled = t0 + TIMING.settle;
        assert_eq!(tick(&mut motion, settled, &target), keys(&["b", "a", "c"]));

        // Second step one interval later reaches the slot.
        let step2 = settled + TIMING.step;
        assert_eq!(tick(&mut motion, step2, &target), keys(&["b", "c", "a"]));
        assert_eq!(motion.next_due(TIMING), None);
    }

    #[test]
    fn no_step_between_intervals() {
        let mut motion = ListMotion::new();
        let t0 = Instant::now();
        tick(&mut motion, t0, &["a", "b", "c"]);
        let target = ["b", "c", "a"];
        let settled = t0 + TIMING.settle;
        tick(&mut motion, t0, &target);
        tick(&mut motion, settled, &target);
        // Half a step later nothing moves yet.
        let mid = settled + TIMING.step / 2;
        assert_eq!(tick(&mut motion, mid, &target), keys(&["b", "a", "c"]));
        assert_eq!(motion.next_due(TIMING), Some(settled + TIMING.step));
    }

    #[test]
    fn reconvergence_cancels_pending_motion() {
        let mut motion = ListMotion::new();
        let t0 = Instant::now();
        tick(&mut motion, t0, &["a", "b", "c"]);
        tick(&mut motion, t0, &["b", "a", "c"]);
        assert!(motion.next_due(TIMING).is_some());

        // Target returns to the display order before settle expires.
        assert_eq!(
            tick(
                &mut motion,
                t0 + Duration::from_millis(500),
                &["a", "b", "c"]
            ),
            keys(&["a", "b", "c"])
        );
        assert_eq!(motion.next_due(TIMING), None);

        // A later divergence starts a fresh settle clock.
        let t1 = t0 + Duration::from_secs(10);
        tick(&mut motion, t1, &["b", "a", "c"]);
        assert_eq!(motion.next_due(TIMING), Some(t1 + TIMING.settle));
    }

    #[test]
    fn upward_move_uses_same_rules() {
        let mut motion = ListMotion::new();
        let t0 = Instant::now();
        tick(&mut motion, t0, &["a", "b", "c"]);

        // "c" starts working and rises to the top.
        let target = ["c", "a", "b"];
        assert_eq!(tick(&mut motion, t0, &target), keys(&["a", "b", "c"]));
        let settled = t0 + TIMING.settle;
        assert_eq!(tick(&mut motion, settled, &target), keys(&["a", "c", "b"]));
        assert_eq!(
            tick(&mut motion, settled + TIMING.step, &target),
            keys(&["c", "a", "b"])
        );
    }

    #[test]
    fn insertions_and_removals_are_instant() {
        let mut motion = ListMotion::new();
        let t0 = Instant::now();
        tick(&mut motion, t0, &["a", "b", "c"]);

        // New key at the top appears immediately; no settle, no steps.
        assert_eq!(
            tick(&mut motion, t0, &["n", "a", "b", "c"]),
            keys(&["n", "a", "b", "c"])
        );
        assert_eq!(motion.next_due(TIMING), None);

        // Removal is immediate too.
        assert_eq!(
            tick(&mut motion, t0, &["n", "a", "c"]),
            keys(&["n", "a", "c"])
        );
        assert_eq!(motion.next_due(TIMING), None);
    }

    #[test]
    fn project_is_pure_and_matches_display_between_ticks() {
        let mut motion = ListMotion::new();
        let t0 = Instant::now();
        tick(&mut motion, t0, &["a", "b", "c"]);
        let target = keys(&["b", "c", "a"]);
        tick(&mut motion, t0, &["b", "c", "a"]);

        // Repeated projections between ticks are identical (hit-testing and
        // render agree) and never mutate.
        let p1 = motion.project(&target);
        let p2 = motion.project(&target);
        assert_eq!(p1, p2);
        assert_eq!(p1, keys(&["a", "b", "c"]));

        // Projection of a target with a brand-new key places it at its slot.
        let with_new = keys(&["n", "b", "c", "a"]);
        assert_eq!(motion.project(&with_new), keys(&["n", "a", "b", "c"]));
    }

    #[test]
    fn mid_flight_retarget_steps_toward_new_target() {
        let mut motion = ListMotion::new();
        let t0 = Instant::now();
        tick(&mut motion, t0, &["a", "b", "c", "d"]);

        // "a" heads for the bottom...
        let down = ["b", "c", "d", "a"];
        tick(&mut motion, t0, &down);
        let settled = t0 + TIMING.settle;
        assert_eq!(
            tick(&mut motion, settled, &down),
            keys(&["b", "a", "c", "d"])
        );

        // ...but then its agent starts working again: new target puts it first.
        // "b" had reached its slot, so it re-arms its own settle hold before
        // yielding — no row moves without having held for the settle delay.
        let up = ["a", "b", "c", "d"];
        let next = settled + TIMING.step;
        assert_eq!(tick(&mut motion, next, &up), keys(&["b", "a", "c", "d"]));
        let released = next + TIMING.settle;
        assert_eq!(
            tick(&mut motion, released, &up),
            keys(&["a", "b", "c", "d"])
        );
        assert_eq!(motion.next_due(TIMING), None);
    }

    #[test]
    fn swap_partner_alignment_clears_bookkeeping() {
        let mut motion = ListMotion::new();
        let t0 = Instant::now();
        tick(&mut motion, t0, &["a", "b"]);

        // Pure swap: one step resolves both keys.
        let target = ["b", "a"];
        tick(&mut motion, t0, &target);
        let settled = t0 + TIMING.settle;
        assert_eq!(tick(&mut motion, settled, &target), keys(&["b", "a"]));
        assert_eq!(motion.next_due(TIMING), None);
    }

    #[test]
    fn reset_snaps_to_target() {
        let mut motion = ListMotion::new();
        let t0 = Instant::now();
        tick(&mut motion, t0, &["a", "b", "c"]);
        tick(&mut motion, t0, &["c", "a", "b"]);
        motion.reset();
        assert_eq!(
            tick(&mut motion, t0, &["c", "a", "b"]),
            keys(&["c", "a", "b"])
        );
        assert_eq!(motion.next_due(TIMING), None);
    }
}
