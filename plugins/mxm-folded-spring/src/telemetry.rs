//! The **only** channel from the audio thread to the editor.
//!
//! Atomics, written once per block, read whenever the editor happens to look. No locks, no
//! allocation, and the UI may drop as many frames as it likes — a display that made the audio
//! thread wait would be a display that could cause a dropout.
//!
//! Two rules carried from every other `telemetry.rs` in the collection:
//!
//! - **A peak is max-combined and reset when the UI reads it.** Overwriting each block means a
//!   transient that landed between two frames is simply gone.
//! - **A clip latches until acknowledged.** Design system §5.4.
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
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

#[derive(Debug)]
pub struct Telemetry {
    /// Peak of the samples produced, max-combined, reset on read.
    peak: AtomicU32,
    /// Sticky: set when a sample reaches full scale, cleared only by the user.
    clipped: AtomicBool,
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
            peak: AtomicU32::new(0),
            clipped: AtomicBool::new(false),
            ring: AtomicU32::new(0),
        }
    }

    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    // ---- audio thread ----

    /// Publish a block's peak. **Combined, not overwritten**: see the module doc.
    pub fn publish_peak(&self, peak: f32) {
        combine(&self.peak, peak);
        if peak >= 1.0 {
            self.clipped.store(true, Ordering::Relaxed);
        }
    }

    /// Publish how loud the wet the block added was.
    pub fn publish_ring(&self, ring: f32) {
        combine(&self.ring, ring);
    }

    // ---- editor thread ----

    /// The loudest sample since this was last called, **and resets**.
    pub fn take_peak(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Ordering::Relaxed))
    }

    /// The loudest wet since this was last called, **and resets**.
    pub fn take_ring(&self) -> f32 {
        f32::from_bits(self.ring.swap(0, Ordering::Relaxed))
    }

    pub fn clipped(&self) -> bool {
        self.clipped.load(Ordering::Relaxed)
    }

    /// Acknowledge the clip indication. The user's act, never a timeout.
    pub fn clear_clip(&self) {
        self.clipped.store(false, Ordering::Relaxed);
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
    fn a_peak_is_combined_and_reset_on_read() {
        let t = Telemetry::new();
        t.publish_peak(0.4);
        t.publish_peak(0.9);
        t.publish_peak(0.2);
        assert_eq!(t.take_peak(), 0.9, "the loudest of the three, not the last");
        assert_eq!(t.take_peak(), 0.0, "and reading resets it");
    }

    #[test]
    fn a_clip_latches_until_acknowledged() {
        let t = Telemetry::new();
        t.publish_peak(1.0);
        for _ in 0..100 {
            t.publish_peak(0.1);
        }
        assert!(t.clipped(), "a meter that forgets is worse than no meter");
        t.clear_clip();
        assert!(!t.clipped());
    }

    #[test]
    fn the_ring_is_combined_and_reset_like_the_peak() {
        let t = Telemetry::new();
        t.publish_ring(0.2);
        t.publish_ring(0.7);
        assert_eq!(t.take_ring(), 0.7);
        assert_eq!(t.take_ring(), 0.0, "a tank that stopped ringing goes dark");
    }
}
