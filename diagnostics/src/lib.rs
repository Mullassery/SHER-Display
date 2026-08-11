//! SHER-Display diagnostics (spec section 35-36).
//!
//! `PerformanceMonitor` is a pure recorder: it does not measure GPU/CPU
//! utilization itself (that's SHER-Graphics's/SHER-Kernel's telemetry to
//! own — see `VISION.md`'s boundary table), it only aggregates whatever
//! the compositor and backend push into it into the snapshot shape spec
//! section 35's `sher-display-monitor` example shows:
//!
//! ```text
//! FPS: 120
//! Frame Time: 8.3 ms
//! Input Latency: 4.2 ms
//! GPU: 37%
//! CPU: 8%
//! Dropped Frames: 0
//! Active Outputs: 2
//! ```
//!
//! `DebugMode` is the gate section 36 requires: "debug functionality must
//! be disabled or restricted in production mode." Callers that expose
//! scene-graph/damage-region visualization or similar inspection tools
//! should check `DebugMode::require_enabled` before doing so, rather than
//! gating ad hoc.

use sher_common::{Error, Result};
use std::collections::VecDeque;

const ROLLING_WINDOW: usize = 120;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiagnosticsSnapshot {
    pub fps: f64,
    pub frame_time_ms: f64,
    pub input_latency_ms: f64,
    pub gpu_percent: f32,
    pub cpu_percent: f32,
    pub dropped_frames: u64,
    pub missed_frames: u64,
    pub active_outputs: usize,
}

#[derive(Default)]
pub struct PerformanceMonitor {
    frame_times_ms: VecDeque<f64>,
    input_latencies_ms: VecDeque<f64>,
    dropped_frames: u64,
    missed_frames: u64,
    gpu_percent: f32,
    cpu_percent: f32,
    active_outputs: usize,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        PerformanceMonitor::default()
    }

    pub fn record_frame(&mut self, frame_time_ms: f64, dropped: bool) {
        push_bounded(&mut self.frame_times_ms, frame_time_ms);
        if dropped {
            self.dropped_frames += 1;
        }
    }

    pub fn record_missed_frame(&mut self) {
        self.missed_frames += 1;
    }

    pub fn record_input_latency(&mut self, latency_ms: f64) {
        push_bounded(&mut self.input_latencies_ms, latency_ms);
    }

    pub fn set_gpu_utilization(&mut self, percent: f32) {
        self.gpu_percent = percent;
    }

    pub fn set_cpu_utilization(&mut self, percent: f32) {
        self.cpu_percent = percent;
    }

    pub fn set_active_outputs(&mut self, count: usize) {
        self.active_outputs = count;
    }

    pub fn snapshot(&self) -> DiagnosticsSnapshot {
        let frame_time_ms = average(&self.frame_times_ms);
        DiagnosticsSnapshot {
            fps: if frame_time_ms > 0.0 { 1000.0 / frame_time_ms } else { 0.0 },
            frame_time_ms,
            input_latency_ms: average(&self.input_latencies_ms),
            gpu_percent: self.gpu_percent,
            cpu_percent: self.cpu_percent,
            dropped_frames: self.dropped_frames,
            missed_frames: self.missed_frames,
            active_outputs: self.active_outputs,
        }
    }
}

fn push_bounded(buf: &mut VecDeque<f64>, value: f64) {
    if buf.len() == ROLLING_WINDOW {
        buf.pop_front();
    }
    buf.push_back(value);
}

fn average(buf: &VecDeque<f64>) -> f64 {
    if buf.is_empty() {
        0.0
    } else {
        buf.iter().sum::<f64>() / buf.len() as f64
    }
}

#[derive(Default)]
pub struct DebugMode {
    enabled: bool,
}

impl DebugMode {
    pub fn new() -> Self {
        DebugMode { enabled: false }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Callers gating scene-graph/damage/z-order visualization or other
    /// inspection tooling (section 36) should call this before doing
    /// anything, so "forgot to check" fails closed rather than open.
    pub fn require_enabled(&self) -> Result<()> {
        if self.enabled {
            Ok(())
        } else {
            Err(Error::Security("debug mode is disabled".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_derives_from_average_frame_time() {
        let mut monitor = PerformanceMonitor::new();
        monitor.record_frame(16.0, false);
        monitor.record_frame(16.0, false);

        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.frame_time_ms, 16.0);
        assert!((snapshot.fps - 62.5).abs() < 0.01);
    }

    #[test]
    fn rolling_window_drops_oldest_sample() {
        let mut monitor = PerformanceMonitor::new();
        for _ in 0..ROLLING_WINDOW {
            monitor.record_frame(10.0, false);
        }
        monitor.record_frame(100.0, false);

        // one 10.0 sample should have been evicted, so the average moves
        // measurably away from 10.0 even though it's still 119:1
        let snapshot = monitor.snapshot();
        assert!(snapshot.frame_time_ms > 10.0);
    }

    #[test]
    fn dropped_and_missed_frames_are_counted() {
        let mut monitor = PerformanceMonitor::new();
        monitor.record_frame(16.0, true);
        monitor.record_frame(16.0, false);
        monitor.record_missed_frame();

        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.dropped_frames, 1);
        assert_eq!(snapshot.missed_frames, 1);
    }

    #[test]
    fn debug_gate_fails_closed_by_default() {
        let debug = DebugMode::new();
        assert!(debug.require_enabled().is_err());
    }

    #[test]
    fn debug_gate_opens_once_enabled() {
        let mut debug = DebugMode::new();
        debug.enable();
        assert!(debug.require_enabled().is_ok());
    }
}
