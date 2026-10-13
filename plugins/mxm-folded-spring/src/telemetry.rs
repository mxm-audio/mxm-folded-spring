//! The **only** channel from the audio thread to the editor.
//!
//! Atomics, written once per block, read whenever the editor happens to look. No locks, no
//! allocation, and the UI may drop as many frames as it likes — a display that made the audio
//! thread wait would be a display that could cause a dropout.
//!
//! **The meter is `mxm_telemetry`'s**, held as `meter`, with one policy for every plugin: a peak
//! is max-combined and reset when the UI reads it, a clip latches until acknowledged (design
//! system §5.4), and a non-finite peak latches the clip and is never the level. What follows here
//! is this plugin's own.
//!
//! # The ring is this plugin's own
//!
//! The display's springs brighten with **how hard the tank is actually ringing** — the wet the
//! audio thread added, after the level and the tank-change fade, not a guess made in the editor
//! from the level control. A tank that has snapped to silence therefore goes dark, which is the
//! one thing the level knob cannot tell you.
//!
//! # No developer channel
//!
//! It arrives as MIDI CC and an effect has no note port; `plugins/AGENTS.md` records why that is a
//! statement about effects rather than an omission here.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug)]
pub struct Telemetry {
    /// The output level and its clip latch: the collection's one meter policy
    /// (`mxm_telemetry::PeakMeter`).
    pub meter: mxm_telemetry::PeakMeter,
    /// The loudest wet the block added, max-combined, reset on read.
    ring: AtomicU32,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            meter: mxm_telemetry::PeakMeter::new(),
            ring: AtomicU32::new(0),
        }
    }

    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    // ---- audio thread ----

    /// Publish how loud the wet the block added was.
    pub fn publish_ring(&self, ring: f32) {
        combine(&self.ring, ring);
    }

    // ---- editor thread ----

    /// The loudest wet since this was last called, **and resets**.
    pub fn take_ring(&self) -> f32 {
        f32::from_bits(self.ring.swap(0, Ordering::Relaxed))
    }
}

/// Max-combines `value` into `slot`, which is an `f32` in its bits.
fn combine(slot: &AtomicU32, value: f32) {
    let mut current = slot.load(Ordering::Relaxed);
    loop {
        let combined = f32::from_bits(current).max(value);
        match slot.compare_exchange_weak(
            current,
            combined.to_bits(),
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(seen) => current = seen,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_is_combined_and_reset_like_the_peak() {
        let t = Telemetry::new();
        t.publish_ring(0.2);
        t.publish_ring(0.7);
        assert_eq!(t.take_ring(), 0.7);
        assert_eq!(t.take_ring(), 0.0, "a tank that stopped ringing goes dark");
    }
}
