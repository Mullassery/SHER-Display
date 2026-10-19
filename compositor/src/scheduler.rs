//! Pure per-output frame scheduling.
//!
//! Each output owns its own refresh interval, next deadline, and tick
//! counter. Advancing the shared clock never changes another output's
//! deadline, so a 60 Hz monitor cannot gate a 144 Hz monitor (or vice
//! versa). This module has no GPU, timer, or thread dependencies: it is
//! driven entirely by caller-supplied `Duration`s, which makes mixed-refresh
//! behaviour testable headlessly.
//!
//! The compositor uses one `FrameScheduler` and polls it from its own loop.
//! Real hardware still needs each output's vblank/presentation completion
//! to feed back into these deadlines (see `ROADMAP.md` Phase 3).

use sher_common::ObjectId;
use std::collections::HashMap;
use std::time::Duration;

/// Refresh interval for a rate in Hz, rounded to the nearest nanosecond.
/// Zero is treated as 1 Hz so a malformed mode can never produce a zero
/// interval (and therefore a division-by-zero or a busy loop).
pub fn refresh_interval(refresh_hz: u32) -> Duration {
    let hz = refresh_hz.max(1) as u64;
    let nanos = (1_000_000_000u64 + hz / 2) / hz;
    Duration::from_nanos(nanos.max(1))
}

/// Per-output timing state. All times are relative to the scheduler's
/// monotonic timeline, not wall-clock time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputTimer {
    pub interval: Duration,
    pub next_deadline: Duration,
    /// Total refresh periods observed since registration. Independent of
    /// every other output's counter.
    pub ticks: u64,
}

/// One output that reached at least one refresh deadline during a poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputTick {
    pub output_id: ObjectId,
    /// Refresh periods that elapsed since the previous poll for this
    /// output. Usually 1; greater than 1 means the caller arrived late
    /// and the scheduler skipped the missed deadlines without emitting a
    /// burst of frames.
    pub elapsed_periods: u64,
    /// This output's total refresh periods since registration.
    pub total_ticks: u64,
    pub next_deadline: Duration,
}

pub struct FrameScheduler {
    now: Duration,
    outputs: HashMap<ObjectId, OutputTimer>,
}

impl FrameScheduler {
    pub fn new() -> Self {
        FrameScheduler {
            now: Duration::ZERO,
            outputs: HashMap::new(),
        }
    }

    pub fn now(&self) -> Duration {
        self.now
    }

    /// Register an output, or reset an existing one to a new refresh rate.
    /// The first deadline is one interval from the current time.
    pub fn register_output(&mut self, output_id: ObjectId, refresh_hz: u32) {
        let interval = refresh_interval(refresh_hz);
        self.outputs.insert(
            output_id,
            OutputTimer {
                interval,
                next_deadline: self.now + interval,
                ticks: 0,
            },
        );
    }

    pub fn unregister_output(&mut self, output_id: &ObjectId) -> bool {
        self.outputs.remove(output_id).is_some()
    }

    /// Change an output's mode/refresh rate without touching any other
    /// output's deadline or counter. The new deadline is one new interval
    /// from now so the output does not immediately fire at the old phase.
    pub fn set_refresh_rate(&mut self, output_id: &ObjectId, refresh_hz: u32) -> bool {
        let now = self.now;
        let Some(timer) = self.outputs.get_mut(output_id) else {
            return false;
        };
        timer.interval = refresh_interval(refresh_hz);
        timer.next_deadline = now + timer.interval;
        true
    }

    pub fn timer(&self, output_id: &ObjectId) -> Option<OutputTimer> {
        self.outputs.get(output_id).copied()
    }

    pub fn next_deadline(&self, output_id: &ObjectId) -> Option<Duration> {
        self.outputs.get(output_id).map(|timer| timer.next_deadline)
    }

    pub fn ticks(&self, output_id: &ObjectId) -> Option<u64> {
        self.outputs.get(output_id).map(|timer| timer.ticks)
    }

    pub fn len(&self) -> usize {
        self.outputs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.outputs.is_empty()
    }

    /// Advance the shared clock by `dt` and report every output whose
    /// deadline is now reached. Deadlines are advanced independently and
    /// missed periods are coalesced into one report per output, so a long
    /// stall cannot cause a frame burst.
    pub fn advance(&mut self, dt: Duration) -> Vec<OutputTick> {
        self.now += dt;
        self.poll()
    }

    /// Advance to an absolute point on the scheduler timeline (monotonic:
    /// moving backwards is ignored) and report due outputs.
    pub fn advance_to(&mut self, now: Duration) -> Vec<OutputTick> {
        if now > self.now {
            self.now = now;
        }
        self.poll()
    }

    /// Report all outputs due at the current time without advancing the
    /// clock. Each output is reported at most once per poll.
    pub fn poll(&mut self) -> Vec<OutputTick> {
        let now = self.now;
        let mut due = Vec::new();

        for (output_id, timer) in self.outputs.iter_mut() {
            if now < timer.next_deadline {
                continue;
            }

            let interval_ns = timer.interval.as_nanos();
            let overdue_ns = (now - timer.next_deadline).as_nanos();
            let periods = 1 + (overdue_ns / interval_ns) as u64;
            let periods_u32 = periods.min(u32::MAX as u64) as u32;

            timer.next_deadline += timer.interval * periods_u32;
            timer.ticks = timer.ticks.saturating_add(periods);

            due.push(OutputTick {
                output_id: *output_id,
                elapsed_periods: periods,
                total_ticks: timer.ticks,
                next_deadline: timer.next_deadline,
            });
        }

        due
    }
}

impl Default for FrameScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    fn tick_for(ticks: &[OutputTick], output_id: ObjectId) -> Option<OutputTick> {
        ticks.iter().find(|t| t.output_id == output_id).copied()
    }

    #[test]
    fn mixed_refresh_rates_have_independent_deadlines() {
        let mut scheduler = FrameScheduler::new();
        let slow = ObjectId::new();
        let fast = ObjectId::new();
        scheduler.register_output(slow, 60);
        scheduler.register_output(fast, 144);

        assert_eq!(scheduler.next_deadline(&slow), Some(refresh_interval(60)));
        assert_eq!(scheduler.next_deadline(&fast), Some(refresh_interval(144)));

        // 7ms reaches the 144 Hz deadline but not the 60 Hz one.
        let due = scheduler.advance(ms(7));
        assert_eq!(due.len(), 1);
        let fast_tick = tick_for(&due, fast).expect("144 Hz output is due at 7ms");
        assert_eq!(fast_tick.elapsed_periods, 1);
        assert_eq!(fast_tick.total_ticks, 1);
        assert!(tick_for(&due, slow).is_none());

        // The slow output's deadline and counter did not move just because
        // the fast output ran.
        let slow_timer = scheduler
            .timer(&slow)
            .expect("slow output stays registered");
        assert_eq!(slow_timer.next_deadline, refresh_interval(60));
        assert_eq!(slow_timer.ticks, 0);

        // Another 10ms (t=17ms) reaches the 60 Hz deadline while the fast
        // output is also due again, independently.
        let due = scheduler.advance(ms(10));
        let slow_tick = tick_for(&due, slow).expect("60 Hz output is due at 17ms");
        let fast_tick = tick_for(&due, fast).expect("144 Hz output is due again at 17ms");
        assert_eq!(slow_tick.total_ticks, 1);
        assert_eq!(fast_tick.total_ticks, 2);
        assert_ne!(slow_tick.next_deadline, fast_tick.next_deadline);
    }

    #[test]
    fn changing_one_output_mode_does_not_move_another_deadline() {
        let mut scheduler = FrameScheduler::new();
        let a = ObjectId::new();
        let b = ObjectId::new();
        scheduler.register_output(a, 60);
        scheduler.register_output(b, 144);
        let b_deadline = scheduler.next_deadline(&b);

        scheduler.advance(ms(5));
        assert!(scheduler.set_refresh_rate(&a, 120));

        assert_eq!(scheduler.next_deadline(&b), b_deadline);
        assert_eq!(scheduler.ticks(&b), Some(0));
        assert_eq!(scheduler.timer(&a).unwrap().interval, refresh_interval(120));
    }

    #[test]
    fn unregistering_one_output_leaves_the_other_running() {
        let mut scheduler = FrameScheduler::new();
        let a = ObjectId::new();
        let b = ObjectId::new();
        scheduler.register_output(a, 60);
        scheduler.register_output(b, 144);

        assert!(scheduler.unregister_output(&a));
        let due = scheduler.advance(ms(7));
        assert_eq!(due.len(), 1);
        assert!(tick_for(&due, b).is_some());
        assert!(scheduler.timer(&a).is_none());
    }

    #[test]
    fn late_poll_coalesces_missed_periods_without_a_frame_burst() {
        let mut scheduler = FrameScheduler::new();
        let output = ObjectId::new();
        scheduler.register_output(output, 144);

        // A 100ms stall spans ~14.4 periods; one report is emitted and the
        // deadline jumps forward to the first future period.
        let due = scheduler.advance(ms(100));
        assert_eq!(due.len(), 1);
        let tick = due[0];
        assert!(tick.elapsed_periods >= 14);
        assert_eq!(tick.total_ticks, tick.elapsed_periods);
        assert!(tick.next_deadline > ms(100));
        assert!(scheduler.advance(Duration::ZERO).is_empty());
    }

    #[test]
    fn headless_simulation_mixed_60_and_144_hz_for_ten_seconds() {
        let mut scheduler = FrameScheduler::new();
        let slow = ObjectId::new();
        let fast = ObjectId::new();
        scheduler.register_output(slow, 60);
        scheduler.register_output(fast, 144);

        // Simulated vblank loop: 1ms steps for ten seconds of display time.
        for _ in 0..10_000 {
            scheduler.advance(ms(1));
        }

        // Expected periods over 10s are 600 (60 Hz) and 1440 (144 Hz).
        // Nanosecond rounding makes the exact boundary off by at most one;
        // the important property is that neither rate was throttled to the
        // other's cadence.
        let slow_ticks = scheduler.ticks(&slow).unwrap();
        let fast_ticks = scheduler.ticks(&fast).unwrap();
        assert!(
            slow_ticks.abs_diff(600) <= 1,
            "60 Hz output ticked {slow_ticks} times in 10s"
        );
        assert!(
            fast_ticks.abs_diff(1440) <= 1,
            "144 Hz output ticked {fast_ticks} times in 10s"
        );
        assert!(fast_ticks > slow_ticks * 2);
    }
}
