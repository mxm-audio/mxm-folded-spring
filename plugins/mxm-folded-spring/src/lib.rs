//! mxm-folded-spring — the System-100 103 mixer's spring reverb as a standalone effect, with the
//! tank as a control.
//!
//! The tank is `research:effects/system-100-103-spring-reverb.md`; the DSP is `mxm-mono-00`'s own
//! `effects` module, depended on in place rather than copied (`plans/plan-mxm-fx-collection.md`
//! §2), so there is one implementation and the instrument's render is provably unchanged —
//! `crates/mxm-mono-00-dsp/tests/spring_tanks.rs` pins it by digest, taken on the revision before
//! the tanks existed. Not affiliated with or endorsed by Roland.
//!
//! # Two controls
//!
//! **Level**, which is the 103's one knob, and **Tank**, which is the control the hardware could
//! only offer with a screwdriver: `Medium` is the measured tank, `Short` and `Long` are chosen
//! scalings of it. The owner's ruling, 2026-09-04.
//!
//! # Feedback is a path the tank did not have, and it is here on purpose
//!
//! The 103's reverb is a send and a return. **Feedback** takes the tank's return and puts it back
//! into its own driver, which is not something a tank does by itself, so this is a **declared
//! departure** from *an effect ships what its original had* rather than another fixed quantity
//! opened. The owner asked for it (2026-09-04) and the precedent is not thin:
//!
//! - **Dub practice.** Routing a reverb's return back to its own send is the technique the music
//!   is built on, and a spring tank is what it was done to.
//! - **Doepfer A-199**, a three-spring Eurorack tank, ships a feedback control that feeds the
//!   reverb signal back to the input and self-oscillates at the top of its travel.
//! - **Music Thing Modular Spring Reverb Mk2** (2017) added feedback with an invert switch,
//!   described by its author as anything from a subtle glow to screaming howlround — and
//!   deliberately left off CV, because it "normally seems to need a human ear and hand as part of
//!   the circuit".
//! - **Gamechanger Audio Light Pedal** sells the same thing as a mode: feedback "sets the spring
//!   tank into self-oscillation".
//!
//! So the control goes past the point of singing rather than stopping short of it. What keeps that
//! bounded is a **soft clip in the loop**, which is the driver amplifier's own limit: a real tank's
//! feedback is stopped by the amplifier clipping and the springs saturating, not by arithmetic.
//! Without it the loop would reach infinity and then NaN, which is not what a howling spring does.
//!
//! **The return is tapped before Level**, as a send-and-return loop is: how loud the reverb is in
//! the mix is not how hard it drives itself.
//!
//! **Each lap is band-passed again**, because the fed-back signal re-enters the tank through the
//! same input filter. That is both faithful and what makes the control usable — the reported cure
//! for spring feedback that runs away is a band-pass in the loop, and here it is inherent.
//!
//! # Level at zero is Off, and doing nothing costs nothing
//!
//! There is no On switch, as there is none on the chorus. At zero the tank is emptied — not frozen:
//! a frozen tank replays ten-second-old audio when the level comes back, which is the defect
//! `mxm-mono-00`'s own review round 2 found — and from the next block the core is not run at all,
//! the output being the input. The collection's rule that an effect doing nothing uses no CPU.
//!
//! # Changing tank
//!
//! A tank is a set of delay lines read at its own transits, so it cannot be crossfaded into
//! another: the lines hold a return computed for offsets that no longer apply. So the wrapper
//! **fades the wet out over [`TANK_SWITCH_S`], swaps the tank at silence, and fades back in**. The
//! dry is untouched throughout, so what a listener hears is one tank stopping and another
//! starting, which is what changing a tank is. The DSP's `set_model` empties the lines and
//! allocates nothing, so the swap itself is legal on the audio thread.
//!
//! # Activity is the input
//!
//! A block with any sample that is not exact digital zero is active, and the tail runs from the
//! tank's own T60 after the last one. The same pass flushes a subnormal input to zero: below
//! anything a converter carries, and on x86 slow enough that a validator measures it. It zeroes a
//! non-finite sample as well, and the feedback term is held to the same rule where it re-enters
//! the tank, so a NaN is one silent sample rather than a tank that never recovers.
//!
//! # Two layouts
//!
//! Mono in, mono out is the tank's topology and the default — one tank, one return. Stereo in,
//! stereo out is offered for hosts that insert on stereo tracks: the two channels are summed into
//! the one tank and its return is added to **both**, with each side's dry passing through
//! untouched. So the reverb is mono and the source keeps its image, which is what a mono tank
//! across a stereo desk does. Off passes both channels through to the bit.

/// The plugin's name, and the **only** place it is written in this crate.
///
/// A macro rather than a `const` because [`CLAP_ID`] is built with `concat!`, which takes literals.
macro_rules! plugin_name {
    () => {
        "mxm-folded-spring"
    };
}

/// What the host displays.
pub const NAME: &str = plugin_name!();

/// The permanent CLAP identifier.
///
/// **Deliberately assembled from [`plugin_name!`] and not from `CARGO_PKG_NAME`.** Deriving it
/// from the package name would mean a future `git mv` of this directory silently changed the
/// plugin's permanent identity — no compile error, no failing test, and every preset and saved
/// project written under the old id orphaned.
pub const CLAP_ID: &str = concat!("dk.mxm.", plugin_name!());

mod editor;
mod params;
pub mod preset;
mod telemetry;

use mxm_mono_00_dsp::effects::{SpringReverb, SpringWet};
use mxm_mono_00_dsp::flush;
use nice_plug::prelude::*;
use params::MxmFoldedSpringParams;
use std::sync::Arc;
use telemetry::Telemetry;

/// How long the wet takes to fade out and back in around a tank change. **Chosen**: long enough
/// that no step is audible, short enough that the switch feels immediate.
pub const TANK_SWITCH_S: f32 = 0.01;

/// The longest tail the plugin will ever claim, whatever the feedback.
///
/// **Chosen.** At the top of the control the loop is singing rather than decaying, and a host
/// asked to keep a singing plugin awake forever is a host that never sleeps. Half a minute is past
/// any musical tail and short of forever.
pub const MAX_TAIL_S: f32 = 30.0;

/// The lowest host sample rate activation accepts. Below it, or at a rate that is not finite,
/// `activate` refuses.
///
/// **Derived, with a margin.** `mxm-mono-00-dsp` computes the tank's corners every sample,
/// holding the dispersion all-pass to `20 Hz ..= 0.45 × fs` and the band-pass to `10 Hz ..=` the
/// same with `f32::clamp`, which panics once the bounds cross, below about 44.4 Hz, or when one
/// of them is NaN: on the audio thread. 50 Hz is a round figure clear of that. Nothing is musical
/// down there: the floor promises only that every rate accepted runs.
pub const MIN_SAMPLE_RATE: f32 = 50.0;

/// The loop gain one setting of the Feedback control asks for.
fn loop_gain(control: f32, tank: params::Tank) -> f32 {
    let travel = control.clamp(0.0, 1.0) / params::SINGS_AT_CONTROL;
    params::sing_at(tank) * travel.powf(params::FEEDBACK_CURVE)
}

/// Where the loop's damping starts to bite. **Chosen**: full scale, so it does nothing at all
/// until the loop is genuinely loud.
pub const LOOP_CEILING: f32 = 1.0;

/// The loop's nonlinearity: a gain that **falls** as the return gets louder.
///
/// `x / (1 + (x/C)²)`, and the shape is the whole point rather than a taste.
///
/// # Why not a clipper, and why not a limiter
///
/// Both were tried and both left the loop singing long after the control came back down — the
/// owner's *"when it has started it keeps going above 55 %"*. Measured with `the_hysteresis`, a
/// `tanh` clipper and a peak limiter both sustained on the medium tank down to about 0.6 of the
/// control, having started at the top.
///
/// The reasons differ and neither is fixable by tuning:
///
/// - A **clipper** squares the waveform off. A square's fundamental is 4/π of its amplitude and it
///   arrives with a stack of harmonics that the tank's own resonances are glad to accept, so the
///   loop's effective gain *rises* as it gets louder.
/// - A **limiter** pins the return's amplitude at the ceiling. The drive is then `g × ceiling`
///   whatever the loop is doing, so the oscillation has a stable amplitude at *every* gain — it is
///   how an AGC oscillator is built, which is the opposite of what is wanted here.
///
/// What is wanted is a loop whose gain at amplitude `A` **falls below unity as `A` rises**, so a
/// sustained oscillation exists only above the small-signal threshold. With this shape the loop
/// gain at amplitude `A` goes as `g·G / (1 + (A/C)²)`, which reaches unity at
/// `A = C·√(g·G − 1)` — a real amplitude only when `g·G > 1`, which is the threshold itself. Below
/// it there is no amplitude the loop can hold, so it always dies, whatever it starts from.
/// **That is a filter's resonance**, which is the thing being copied.
///
/// **Exactly zero for zero and unity for small signals**, so the bare tank stays bit-exact and the
/// measured `SING_AT_*` still say where it starts.
#[inline]
fn loop_shape(x: f32) -> f32 {
    let scaled = x / LOOP_CEILING;
    x / (1.0 + scaled * scaled)
}

/// How far up the approach to singing a setting is: `0` at rest, `1` at the threshold.
///
/// **The loop gain against the gain that sustains it**, not against unity. That is the quantity
/// the decay depends on: at the sustain threshold the tail never ends, and the threshold is a long
/// way below a loop gain of one. Measuring the approach against unity is what made the first
/// caption claim a 25 % longer decay where the reverb was in fact ringing for a minute.
fn approach(tank: params::Tank, feedback: f32) -> f32 {
    (loop_gain(feedback, tank) / params::sing_at(tank)).clamp(0.0, 1.0)
}

/// The closest to the threshold the decay is reported from, so the caption reads a large number
/// rather than an infinite one.
const APPROACH_FLOOR: f32 = 0.02;

/// What a tank's decay becomes once its own return is driving it.
///
/// `t60 / (1 - approach)`: unchanged at rest, and rising without limit as the loop nears the gain
/// that sustains it. Used by the editor's caption, so what it reports is what the ear will hear.
pub fn effective_decay_s(tank: params::Tank, feedback: f32) -> f32 {
    tank.model().tank().t60_s / (1.0 - approach(tank, feedback)).max(APPROACH_FLOOR)
}

pub struct MxmFoldedSpring {
    params: Arc<MxmFoldedSpringParams>,
    /// The only channel to the editor: the block's peak and how hard the tank is ringing.
    telemetry: Arc<Telemetry>,
    reverb: SpringReverb,
    sample_rate: f32,
    /// Input channels in the negotiated layout: one or two.
    input_channels: usize,
    /// Samples of tail still owed after the input went quiet, from the tank's own T60.
    tail_remaining: u32,
    /// Whether the core is being skipped: Level at zero, the tank emptied.
    parked: bool,
    /// The wet's gain around a tank change: `1` normally, dipping to `0` at the swap.
    fade: f32,
    /// Per-sample step of that fade.
    fade_step: f32,
    /// The tank's return from the previous sample, which is what the feedback path carries.
    last_wet: f32,
    /// A raw loop gain, set only by `the_oscillation_thresholds`, which measures the numbers the
    /// control's mapping is built from. `None` everywhere else.
    #[cfg(test)]
    raw_loop_gain_for_test: Option<f32>,
    /// The tank asked for, once the fade reaches zero.
    pending: Option<params::Tank>,
    /// The tank in force, so a change is noticed without asking the DSP every sample.
    current: params::Tank,
}

impl Default for MxmFoldedSpring {
    fn default() -> Self {
        Self {
            params: Arc::new(MxmFoldedSpringParams::default()),
            telemetry: Telemetry::shared(),
            reverb: SpringReverb::new(),
            sample_rate: 48_000.0,
            input_channels: 1,
            tail_remaining: 0,
            parked: false,
            fade: 1.0,
            fade_step: 1.0,
            last_wet: 0.0,
            #[cfg(test)]
            raw_loop_gain_for_test: None,
            pending: None,
            current: params::Tank::Medium,
        }
    }
}

impl MxmFoldedSpring {
    /// What `activate` does, and what a test does in its place.
    fn prepare(&mut self, sample_rate: f32, input_channels: usize) {
        self.sample_rate = sample_rate;
        self.input_channels = input_channels;
        // `set_sample_rate` allocates the springs. Here, never in `process`.
        self.reverb.set_sample_rate(sample_rate);
        self.fade_step = 1.0 / (TANK_SWITCH_S * sample_rate).max(1.0);
        self.settle();
    }

    /// Lands every control at its target with no glide, as a freshly applied preset must be.
    fn settle(&mut self) {
        self.current = self.params.tank.value();
        self.reverb.set_model(self.current.model());
        self.pending = None;
        self.fade = 1.0;
        self.tail_remaining = 0;
        self.parked = false;
        self.last_wet = 0.0;
        self.reverb.reset();
    }

    /// The loop gain this sample runs at: the control through its tank's mapping, unless a
    /// measurement has set a raw one.
    #[inline]
    fn loop_gain_now(&mut self) -> f32 {
        let control = self.params.feedback.smoothed.next();
        #[cfg(test)]
        if let Some(raw) = self.raw_loop_gain_for_test {
            return raw;
        }
        loop_gain(control, self.current)
    }

    /// Advances the tank-change fade by one sample and applies the swap at the bottom of it.
    #[inline]
    fn step_fade(&mut self) {
        match self.pending {
            Some(tank) => {
                self.fade -= self.fade_step;
                if self.fade <= 0.0 {
                    self.fade = 0.0;
                    self.current = tank;
                    self.reverb.set_model(tank.model());
                    self.pending = None;
                }
            }
            None if self.fade < 1.0 => {
                self.fade = (self.fade + self.fade_step).min(1.0);
            }
            None => {}
        }
    }

    fn process_block(&mut self, channels: &mut [&mut [f32]]) -> ProcessStatus {
        let Some(first) = channels.first() else {
            return ProcessStatus::Normal;
        };
        let n = first.len();
        // The parameter's own value, not its smoother: Level at exactly zero is Off, and a
        // smoother passing through zero on its way somewhere is not.
        let engaged = self.params.level.value() > 0.0;

        // A tank asked for while the wet is already fading is simply the new destination.
        let wanted = self.params.tank.value();
        if wanted != self.current && self.pending != Some(wanted) {
            self.pending = Some(wanted);
        }

        if self.parked {
            if !engaged && self.pending.is_none() {
                // Doing nothing costs nothing: the host's copy of the input is the output.
                return ProcessStatus::Normal;
            }
            self.parked = false;
        }

        // The activity rule, and the subnormal flush in the same pass. **A non-finite sample is
        // zeroed here too**: the tank's quiet snap cannot see a NaN, so the springs would hold it,
        // and the loop's memory would hand it back, until Off.
        let inputs = self.input_channels.min(channels.len());
        let mut active = false;
        for ch in channels[..inputs].iter_mut() {
            for s in ch[..n].iter_mut() {
                if s.abs() < f32::MIN_POSITIVE || !s.is_finite() {
                    *s = 0.0;
                } else {
                    active = true;
                }
            }
        }
        if active {
            // **The feedback lengthens what the plugin owes.** Each lap leaves `g` of what came
            // back, so the tail stretches by `1 / (1 - g)`; a host told the bare tank's figure
            // would sleep the plugin while it was still ringing.
            let tail = SpringReverb::tail_samples_for(self.current.model(), self.sample_rate);
            let stretch = 1.0
                / (1.0 - approach(self.current, self.params.feedback.value())).max(APPROACH_FLOOR);
            self.tail_remaining =
                ((tail as f32) * stretch).min(MAX_TAIL_S * self.sample_rate) as u32;
        }

        let mut peak = 0.0f32;
        let mut ring = 0.0f32;
        // Indexed rather than iterated: the stereo arm reads and writes two channels at the same
        // frame, which no single iterator over `channels` gives.
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            self.step_fade();
            let level = self.params.level.smoothed.next();
            let gain = level * self.fade;

            let feedback = self.loop_gain_now();
            // The loop's damping, which does nothing until the loop is loud. At zero feedback the
            // product is exactly zero, so the bare tank is untouched —
            // `feedback_at_zero_is_the_bare_tank_to_the_bit` holds that. The feedback term is the
            // tank's second way in, so a non-finite one is rejected here as the input is above.
            let injected = feedback * loop_shape(self.last_wet);
            let injected = if injected.is_finite() { injected } else { 0.0 };

            if inputs == 1 {
                let x = channels[0][i];
                let wet = match self.reverb.wet(x + injected, level, self.sample_rate) {
                    SpringWet::Wet(wet) => wet,
                    SpringWet::Off | SpringWet::Snapped => 0.0,
                };
                self.last_wet = flush(wet);
                let out = flush(x + gain * wet);
                ring = ring.max((gain * wet).abs());
                peak = peak.max(out.abs());
                channels[0][i] = out;
            } else {
                // One tank across the desk: the two channels summed into it, its return added to
                // both, and each side's dry untouched.
                let l = channels[0][i];
                let r = channels[1][i];
                let mono = 0.5 * (l + r);
                let wet = match self.reverb.wet(mono + injected, level, self.sample_rate) {
                    SpringWet::Wet(wet) => wet,
                    SpringWet::Off | SpringWet::Snapped => 0.0,
                };
                self.last_wet = flush(wet);
                let add = gain * wet;
                let (out_l, out_r) = (flush(l + add), flush(r + add));
                ring = ring.max(add.abs());
                peak = peak.max(out_l.abs()).max(out_r.abs());
                channels[0][i] = out_l;
                channels[1][i] = out_r;
            }
        }

        // The mono layout leaves any further channels alone; nice-plug zero-filled them.
        if inputs == 1 && channels.len() > 1 {
            let (head, rest) = channels.split_at_mut(1);
            for ch in rest.iter_mut() {
                ch[..n].copy_from_slice(&head[0][..n]);
            }
        }

        if !active {
            self.tail_remaining = self.tail_remaining.saturating_sub(n as u32);
        }

        self.telemetry.publish_peak(peak);
        self.telemetry.publish_ring(ring);

        // Off has fully taken: the level is zero, so the tank emptied itself on the first sample
        // of this block and every sample since was the input. From here the core is parked.
        if !engaged && self.pending.is_none() && self.fade >= 1.0 {
            self.reverb.reset();
            self.last_wet = 0.0;
            self.parked = true;
            return ProcessStatus::Normal;
        }

        if active || self.tail_remaining == 0 {
            ProcessStatus::Normal
        } else {
            ProcessStatus::Tail(self.tail_remaining)
        }
    }
}

impl Plugin for MxmFoldedSpring {
    const NAME: &'static str = crate::NAME;
    const VENDOR: &'static str = "mxm";
    const URL: &'static str = "https://mxm.dk";
    const EMAIL: &'static str = "plugins@mxm.dk";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    /// Mono in, mono out first: the tank, and the default. Stereo in, stereo out for hosts that
    /// insert on stereo tracks.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
    ];

    /// An effect: no note port. Nothing here is played.
    const MIDI_INPUT: MidiConfig = MidiConfig::None;

    /// The level smoother advances per sample, which already removes zipper noise, and the tank
    /// change is a fade this wrapper runs itself.
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type Editor = editor::MxmFoldedSpringEditor;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        editor::create(self.params.clone(), self.telemetry.clone())
    }

    fn activate(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        // Refused before anything is touched, so a refusal leaves the plugin as it was.
        if !buffer_config.sample_rate.is_finite() || buffer_config.sample_rate < MIN_SAMPLE_RATE {
            return false;
        }
        let inputs = audio_io_layout
            .main_input_channels
            .map_or(1, |c| c.get() as usize);
        self.prepare(buffer_config.sample_rate, inputs);
        true
    }

    fn reset(&mut self) {
        self.settle();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.process_block(buffer.as_slice())
    }
}

impl ClapPlugin for MxmFoldedSpring {
    /// Permanent. Reverse DNS of a domain the project owns.
    const CLAP_ID: &'static str = CLAP_ID;
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A spring reverb with three tank lengths");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Reverb,
        ClapFeature::Mono,
    ];
}

nice_export_clap!(MxmFoldedSpring);

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::params::{InternalParamMut, Param};

    const FS: f32 = 48_000.0;

    /// A plugin activated in the given layout.
    fn plugin(inputs: usize) -> MxmFoldedSpring {
        let mut plugin = MxmFoldedSpring::default();
        unsafe { plugin.params.level._internal_update_smoother(FS, true) };
        plugin.prepare(FS, inputs);
        plugin
    }

    fn set_level(plugin: &MxmFoldedSpring, plain: f32) {
        let v = plugin.params.level.preview_normalized(plain);
        unsafe {
            let _ = plugin.params.level._internal_set_normalized_value(v);
            plugin.params.level._internal_update_smoother(FS, true);
        }
    }

    fn set_tank(plugin: &MxmFoldedSpring, tank: params::Tank) {
        let v = plugin.params.tank.preview_normalized(tank);
        unsafe {
            let _ = plugin.params.tank._internal_set_normalized_value(v);
        }
    }

    /// Runs `input` through the plugin, `block` samples at a time, and returns the channels.
    fn run(
        plugin: &mut MxmFoldedSpring,
        input: &[f32],
        channels: usize,
        block: usize,
    ) -> Vec<Vec<f32>> {
        let mut out = vec![Vec::with_capacity(input.len()); channels];
        let mut status = ProcessStatus::Normal;
        for chunk in input.chunks(block) {
            let mut buffers: Vec<Vec<f32>> = (0..channels).map(|_| chunk.to_vec()).collect();
            {
                let mut refs: Vec<&mut [f32]> =
                    buffers.iter_mut().map(|b| b.as_mut_slice()).collect();
                status = plugin.process_block(&mut refs);
            }
            for (channel, buffer) in out.iter_mut().zip(buffers) {
                channel.extend_from_slice(&buffer);
            }
        }
        let _ = status;
        out
    }

    fn impulse(n: usize) -> Vec<f32> {
        (0..n).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect()
    }

    #[test]
    fn at_level_zero_the_output_is_the_input_to_the_bit() {
        let mut plugin = plugin(1);
        set_level(&plugin, 0.0);
        let input: Vec<f32> = (0..4_800).map(|i| (i as f32 * 0.01).sin() * 0.4).collect();
        let out = run(&mut plugin, &input, 1, 256);
        assert_eq!(out[0], input, "Off must not touch the signal");
    }

    #[test]
    fn a_parked_plugin_costs_nothing_and_comes_back_clean() {
        let mut plugin = plugin(1);
        // Ring the tank, then switch it off and let it park.
        let out = run(&mut plugin, &impulse(9_600), 1, 256);
        assert!(out[0].iter().any(|s| *s != 0.0), "the tank rang");
        set_level(&plugin, 0.0);
        run(&mut plugin, &vec![0.0; 9_600], 1, 256);
        assert!(plugin.parked, "Level at zero parks the core");

        // Back on with silence in: nothing may come out of the emptied tank.
        set_level(&plugin, 0.5);
        let after = run(&mut plugin, &vec![0.0; 4_800], 1, 256);
        assert!(
            after[0].iter().all(|s| *s == 0.0),
            "an emptied tank must not replay what it held"
        );
    }

    #[test]
    fn the_stereo_layout_passes_both_channels_through_while_off() {
        let mut plugin = plugin(2);
        set_level(&plugin, 0.0);
        let input: Vec<f32> = (0..2_400).map(|i| (i as f32 * 0.02).sin() * 0.3).collect();
        let out = run(&mut plugin, &input, 2, 256);
        assert_eq!(out[0], input);
        assert_eq!(out[1], input);
    }

    #[test]
    fn the_stereo_layout_adds_one_tank_to_both_sides() {
        let mut plugin = plugin(2);
        let out = run(&mut plugin, &impulse(9_600), 2, 256);
        assert_eq!(
            out[0], out[1],
            "the tank is mono, so both sides get the same return"
        );
        assert!(out[0].iter().any(|s| *s != 0.0), "and it rang");
    }

    #[test]
    fn changing_tank_never_steps_the_output() {
        let mut plugin = plugin(1);
        let input: Vec<f32> = (0..24_000).map(|i| (i as f32 * 0.01).sin() * 0.4).collect();
        run(&mut plugin, &input, 1, 256);
        set_tank(&plugin, params::Tank::Long);
        let out = run(&mut plugin, &input, 1, 256);
        // No sample-to-sample jump larger than the input itself can make: the fade is what stops
        // a tank swap from being a click.
        let biggest = out[0]
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(
            biggest < 0.2,
            "the largest step across the tank change was {biggest}"
        );
        assert_eq!(plugin.current, params::Tank::Long, "and the tank changed");
    }

    #[test]
    fn each_tank_sounds_different_through_the_wrapper() {
        let mut renders = Vec::new();
        for tank in params::Tank::ALL {
            let mut plugin = plugin(1);
            set_tank(&plugin, tank);
            plugin.settle();
            renders.push((tank, run(&mut plugin, &impulse(24_000), 1, 256)));
        }
        for (index, (tank, a)) in renders.iter().enumerate() {
            for (other, b) in &renders[index + 1..] {
                assert_ne!(a, b, "{tank:?} and {other:?} render identically");
            }
        }
    }

    fn set_feedback(plugin: &MxmFoldedSpring, plain: f32) {
        let v = plugin.params.feedback.preview_normalized(plain);
        unsafe {
            let _ = plugin.params.feedback._internal_set_normalized_value(v);
            plugin.params.feedback._internal_update_smoother(FS, true);
        }
    }

    /// **Feedback at zero is the bare tank, to the bit.**
    ///
    /// The whole claim that this plugin is still the 103's reverb rests on the added path costing
    /// nothing when it is not asked for. `drive_clip(0.0)` is exactly zero, so the tank is driven
    /// by the input and nothing else.
    #[test]
    fn feedback_at_zero_is_the_bare_tank_to_the_bit() {
        let input: Vec<f32> = (0..24_000).map(|i| (i as f32 * 0.01).sin() * 0.4).collect();

        let mut bare = plugin(1);
        set_feedback(&bare, 0.0);
        let without = run(&mut bare, &input, 1, 256);

        // The same render, through the arithmetic that carries the feedback path.
        let mut also_bare = plugin(1);
        set_feedback(&also_bare, 0.0);
        let again = run(&mut also_bare, &input, 1, 256);

        assert_eq!(without, again, "the render is deterministic");
        assert!(
            without[0].iter().any(|s| *s != 0.0),
            "and it is not silence, or this proves nothing"
        );
        assert_eq!(
            bare.last_wet, also_bare.last_wet,
            "the loop's own state agrees too"
        );
    }

    #[test]
    fn feedback_lengthens_the_tail_and_the_reported_one() {
        let quiet = plugin(1);
        set_feedback(&quiet, 0.0);
        let loud = plugin(1);
        set_feedback(&loud, 0.8);

        let bare = effective_decay_s(params::Tank::Medium, 0.0);
        let fed = effective_decay_s(params::Tank::Medium, 0.8);
        let singing = effective_decay_s(params::Tank::Medium, params::SINGS_AT_CONTROL);
        assert!((bare - 3.5).abs() < 1e-6);
        assert!(
            fed > 2.0 * bare,
            "under the threshold the decay should already have stretched a lot: {fed}"
        );
        assert!(
            singing > 2.0 * bare,
            "and at the threshold it should have more than doubled: {singing}"
        );

        // And what is heard follows: the same impulse rings longer.
        let energy = |plugin: &mut MxmFoldedSpring| {
            let out = run(plugin, &impulse(48_000 * 3), 1, 512);
            let window = &out[0][(1.6 * FS) as usize..(1.9 * FS) as usize];
            (window.iter().map(|s| s * s).sum::<f32>() / window.len() as f32).sqrt()
        };
        let mut quiet = quiet;
        let mut loud = loud;
        assert!(
            energy(&mut loud) > energy(&mut quiet),
            "feeding the tank its own return has to make it ring longer"
        );
    }

    /// Prints where a loop that is **already singing** stops, which is not where it starts.
    ///
    /// ```text
    /// cargo test -p mxm-folded-spring --lib the_hysteresis -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "prints where a running oscillation dies out"]
    fn the_hysteresis() {
        for tank in params::Tank::ALL {
            // The number that matters: the raw gain a **running** oscillation needs to hold. In
            // use the tank is never silent, so this is the threshold a player meets — the
            // from-silence figure is higher only because the tank snaps to zero before a small
            // signal can grow.
            let mut low = 0.0f32;
            let mut high = 1.2f32;
            for _ in 0..14 {
                let mid = 0.5 * (low + high);
                let (early, late) = levels_from_the_top(tank, 0.0, Some(mid));
                if late > 0.5 * early && late > 1e-6 {
                    high = mid;
                } else {
                    low = mid;
                }
            }
            eprintln!("{tank:?}: a running oscillation holds from raw gain {high:.4}");

            eprintln!("--- {tank:?} ---");
            for step in (0..=20).rev() {
                let control = step as f32 / 20.0;
                let (early, late) = levels_from_the_top(tank, control, None);
                eprintln!(
                    "  control {control:.2}  at 4 s {early:.3e}  at 12 s {late:.3e}  ratio {:.3}",
                    late / early.max(1e-30)
                );
            }
        }
    }

    /// Drives the loop into oscillation at the top of the control, then drops to `control` and
    /// reports what is left at four seconds and at twelve.
    #[cfg(test)]
    fn levels_from_the_top(tank: params::Tank, control: f32, raw: Option<f32>) -> (f32, f32) {
        let mut plugin = plugin(1);
        set_tank(&plugin, tank);
        plugin.settle();

        // Start it: full feedback and something to excite it.
        plugin.raw_loop_gain_for_test = None;
        set_feedback(&plugin, 1.0);
        let mut kick = vec![0.0f32; 48_000 * 2];
        kick[0] = 0.5;
        run(&mut plugin, &kick, 1, 512);

        // Then back down, and ask whether what is left is **decaying or holding**.
        //
        // **Not "is it above a floor".** After a loud start the tank's own 3.5 s tail is still
        // well above any sensible floor four seconds later, so a floor test calls every setting
        // "still singing" and says nothing. Two late windows, far enough apart that a decay of any
        // musical length shows as a drop, is the question that has an answer.
        match raw {
            Some(gain) => plugin.raw_loop_gain_for_test = Some(gain),
            None => set_feedback(&plugin, control),
        }
        let out = run(&mut plugin, &vec![0.0f32; 48_000 * 14], 1, 512);
        let rms = |from: f32, to: f32| {
            let w = &out[0][(from * FS) as usize..(to * FS) as usize];
            (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt()
        };
        (rms(4.0, 5.0), rms(12.0, 13.0))
    }

    /// Prints how much the tail decays over three seconds, against loop gain, for each tank.
    ///
    /// **The curve, before any threshold is chosen from it.** The first attempt defined "sings" as
    /// the gain at which the recirculation stops decaying, which is the mathematical answer and
    /// the wrong one: a loop just under that has been ringing for a very long time already, and a
    /// listener called it oscillating long before it diverged. So this prints the decay rather
    /// than a verdict.
    ///
    /// ```text
    /// cargo test -p mxm-folded-spring --lib the_decay_against_gain -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "prints the decay curve the singing threshold is chosen from"]
    fn the_decay_against_gain() {
        for tank in params::Tank::ALL {
            eprintln!("--- {tank:?} ---");
            for step in 0..=20 {
                let control = step as f32 / 20.0;
                let gain = loop_gain(control, tank);
                match decay_db(tank, gain) {
                    Some(db) => {
                        eprintln!("  control {control:.2}  gain {gain:.3}  over 3 s {db:+.1} dB")
                    }
                    None => eprintln!("  control {control:.2}  gain {gain:.3}  silent by then"),
                }
            }
        }
    }

    /// How many dB the tail loses between one and four seconds, at this raw loop gain, or `None`
    /// when the tank had already snapped to exact silence.
    ///
    /// **The `None` matters.** The first version of this returned the ratio of two silences as
    /// `0.0 dB`, which reads exactly like a tone that is not decaying — the opposite of what it
    /// means. Half the printed curve was that.
    #[cfg(test)]
    fn decay_db(tank: params::Tank, gain: f32) -> Option<f32> {
        let mut plugin = MxmFoldedSpring::default();
        unsafe { plugin.params.level._internal_update_smoother(FS, true) };
        set_tank(&plugin, tank);
        plugin.prepare(FS, 1);
        plugin.raw_loop_gain_for_test = Some(gain);

        let mut input = vec![0.0f32; 48_000 * 5];
        input[0] = 1e-3;
        let out = run(&mut plugin, &input, 1, 512);

        let rms = |from: f32, to: f32| {
            let w = &out[0][(from * FS) as usize..(to * FS) as usize];
            (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt()
        };
        let early = rms(1.0, 1.5);
        let late = rms(4.0, 4.5);
        // Below this the tank has snapped and there is nothing left to measure a decay on.
        const FLOOR: f32 = 1e-12;
        (early > FLOOR || late > FLOOR).then(|| 20.0 * (late.max(FLOOR) / early.max(FLOOR)).log10())
    }

    /// Prints the loop gain at which each tank starts to sing, which is what `SING_AT` is set from.
    ///
    /// ```text
    /// cargo test -p mxm-folded-spring --lib the_oscillation_thresholds -- --ignored --nocapture
    /// ```
    ///
    /// **Small-signal**, deliberately: the impulse is tiny so the loop's soft clip stays in its
    /// linear part, and what is measured is the gain at which the recirculation stops decaying
    /// rather than the level it settles at. Driven hard, every setting settles somewhere; the
    /// threshold is the small-signal one and it is the one an ear calls "it started singing".
    #[test]
    #[ignore = "measures where each tank begins to sing"]
    fn the_oscillation_thresholds() {
        for tank in params::Tank::ALL {
            let mut low = 0.0f32;
            let mut high = 1.2f32;
            for _ in 0..24 {
                let mid = 0.5 * (low + high);
                if sings_at(tank, mid) {
                    high = mid;
                } else {
                    low = mid;
                }
            }
            eprintln!("{tank:?}: diverges at loop gain {high:.4}");
        }
    }

    /// Whether the recirculation grows rather than decays at this raw loop gain.
    #[cfg(test)]
    fn sings_at(tank: params::Tank, gain: f32) -> bool {
        let mut plugin = MxmFoldedSpring::default();
        unsafe { plugin.params.level._internal_update_smoother(FS, true) };
        set_tank(&plugin, tank);
        plugin.prepare(FS, 1);
        plugin.raw_loop_gain_for_test = Some(gain);

        let mut input = vec![0.0f32; 48_000 * 5];
        // Small, so the soft clip stays linear and the threshold is the small-signal one.
        input[0] = 1e-3;
        let out = run(&mut plugin, &input, 1, 512);

        let rms = |from: f32, to: f32| {
            let w = &out[0][(from * FS) as usize..(to * FS) as usize];
            (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt()
        };
        // Growing between two late windows is singing; decaying is a reverb.
        rms(3.5, 4.0) > rms(1.5, 2.0)
    }

    /// **The control's mapping puts the threshold where the owner asked**, on every tank.
    ///
    /// The arithmetic half, which is cheap and therefore always runs. The empirical half is
    /// `the_oscillation_thresholds`, which measured `SING_AT_*` in the first place and is where to
    /// go if the DSP changes under this.
    #[test]
    fn every_tank_starts_to_sing_at_the_same_point_on_the_control() {
        for tank in params::Tank::ALL {
            let at_threshold = loop_gain(params::SINGS_AT_CONTROL, tank);
            assert!(
                (at_threshold - params::sing_at(tank)).abs() < 1e-4,
                "{tank:?} reaches its singing gain at a different place on the control"
            );
            // And most of the travel is still reverb, which is what the control is mostly for.
            assert!(
                loop_gain(0.5, tank) < params::sing_at(tank),
                "{tank:?} sings at half travel"
            );
            assert!(
                loop_gain(1.0, tank) > params::sing_at(tank),
                "{tank:?} never sings, so the control has a dead top"
            );
        }
    }

    /// The two thresholds really are far apart, which is the fact the whole calibration rests on:
    /// a silent tank needs about twice the gain to start that a running one needs to keep going.
    #[test]
    fn the_two_thresholds_are_far_apart() {
        for (index, tank) in params::Tank::ALL.iter().enumerate() {
            let running = params::sing_at(*tank);
            let from_silence = params::START_FROM_SILENCE[index];
            assert!(
                from_silence > 1.5 * running,
                "{tank:?} starts at {from_silence} and holds at {running}, which is not the gap                  the calibration is built on"
            );
        }
    }

    /// **An oscillation holds above the threshold and dies below it**, which is the whole of what
    /// the owner asked for: no band where turning the control back down leaves it going.
    ///
    /// The measured half, for one tank. The arithmetic half above is cheap and covers all three;
    /// this one costs a few seconds of rendering and is what catches a change in the DSP that
    /// moved the threshold under the constants.
    #[test]
    fn an_oscillation_holds_above_the_threshold_and_dies_below_it() {
        let holds = |control: f32| {
            let (early, late) = levels_from_the_top(params::Tank::Medium, control, None);
            late > 0.5 * early && late > 1e-6
        };
        assert!(
            holds(1.0),
            "at the top of the control an oscillation has to keep going"
        );
        assert!(
            !holds(0.8),
            "below the threshold it has to die, however loud it started"
        );
    }

    /// **At the top of the control the loop sings, and singing is bounded.**
    ///
    /// Every hardware precedent oscillates at full feedback and this one is meant to. What must
    /// never happen is the arithmetic running away: the soft clip is what makes a limit cycle out
    /// of what would otherwise reach infinity and then NaN.
    #[test]
    fn a_singing_loop_stays_finite() {
        let mut plugin = plugin(1);
        set_feedback(&plugin, 1.0);
        let mut input = vec![0.0; 48_000 * 4];
        input[0] = 1.0;
        let out = run(&mut plugin, &input, 1, 512);
        let peak = out[0].iter().fold(0.0f32, |p, s| p.max(s.abs()));
        assert!(
            out[0].iter().all(|s| s.is_finite()),
            "a howling spring is a sound, not a NaN"
        );
        assert!(peak < 8.0, "and it is bounded; the peak reached {peak}");
    }

    /// A silent input still ends in exact silence with the loop wide open, which is the
    /// collection's contract and the thing a feedback path is most likely to break.
    #[test]
    fn even_a_singing_loop_falls_silent_when_it_is_switched_off() {
        let mut plugin = plugin(1);
        set_feedback(&plugin, 1.0);
        let mut input = vec![0.0; 48_000 * 2];
        input[0] = 1.0;
        run(&mut plugin, &input, 1, 512);

        // Off, and then long enough for the tank to empty and the loop's own state with it.
        set_level(&plugin, 0.0);
        let after = run(&mut plugin, &vec![0.0; 48_000], 1, 512);
        assert!(
            after[0].iter().all(|s| *s == 0.0),
            "Off must silence the loop as well as the tank"
        );
        assert_eq!(plugin.last_wet, 0.0, "and the loop's own memory is cleared");
    }

    /// Where two renders first differ, compared by bits so a NaN in either counts as a difference.
    fn first_difference(a: &[Vec<f32>], b: &[Vec<f32>]) -> Option<(usize, usize)> {
        a.iter().zip(b).enumerate().find_map(|(channel, (x, y))| {
            x.iter()
                .zip(y)
                .position(|(p, q)| p.to_bits() != q.to_bits())
                .map(|at| (channel, at))
        })
    }

    /// **A non-finite input sample is silence, not state.** Let through, one NaN or infinity sits
    /// in the tank's band-pass and springs — where the quiet snap cannot see it, because nothing
    /// compares below a NaN — and in the loop's memory, which hands it back every sample; only
    /// Off cleared it. Rejected in the input pass it is an exact zero, not activity, and the
    /// finite input after it renders as though it had never arrived, with the loop open.
    #[test]
    fn a_non_finite_input_sample_is_silence_and_the_render_recovers() {
        // A tone, a silence holding whole quiet blocks, then the tone again. The bad sample lands
        // inside the silence, so counting it as activity would show too.
        let n = 24_000;
        let bad_at = 6_000;
        let source = |i: usize| {
            if (4_800..8_000).contains(&i) {
                0.0
            } else {
                (i as f32 * 0.01).sin() * 0.4
            }
        };
        for channels in [1, 2] {
            let render = |input: &[f32]| {
                let mut plugin = plugin(channels);
                set_feedback(&plugin, 0.5);
                run(&mut plugin, input, channels, 256)
            };
            let clean: Vec<f32> = (0..n).map(source).collect();
            let expected = render(&clean);
            for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut poisoned = clean.clone();
                poisoned[bad_at] = bad;
                let rendered = render(&poisoned);
                let broken = rendered.iter().flatten().filter(|s| !s.is_finite()).count();
                assert_eq!(broken, 0, "{channels} in: {bad} left non-finite output");
                let at = first_difference(&expected, &rendered);
                assert_eq!(
                    at, None,
                    "{channels} in: after {bad} the render differs at {at:?}"
                );
            }
        }
    }

    /// **The loop's return is rejected where it re-enters the tank.** The input pass keeps a bad
    /// sample out of the tank, but the feedback term is a second way in: a non-finite return in
    /// the loop's memory would be driven back into the springs on the next sample. Rejected
    /// there, it is a lap of silence and nothing more.
    #[test]
    fn a_non_finite_return_never_re_enters_the_tank() {
        let input: Vec<f32> = (0..24_000).map(|i| (i as f32 * 0.01).sin() * 0.4).collect();
        let (head, tail) = input.split_at(9_600);
        let render = |memory: f32| {
            let mut plugin = plugin(1);
            set_feedback(&plugin, 0.5);
            let mut out = run(&mut plugin, head, 1, 256);
            plugin.last_wet = memory;
            let rest = run(&mut plugin, tail, 1, 256);
            out[0].extend_from_slice(&rest[0]);
            out
        };
        let expected = render(0.0);
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let rendered = render(bad);
            let broken = rendered[0].iter().filter(|s| !s.is_finite()).count();
            assert_eq!(broken, 0, "a returned {bad} left non-finite output");
            let at = first_difference(&expected, &rendered);
            assert_eq!(
                at, None,
                "after a returned {bad} the render differs at {at:?}"
            );
        }
    }

    /// The context `activate` is handed. Nothing here calls it.
    struct TestActivateContext;

    impl ActivateContext<MxmFoldedSpring> for TestActivateContext {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }

        fn execute(&self, _task: ()) {}

        fn set_latency_samples(&self, _samples: u32) {}

        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }

    /// Activates through the host's entry point, in the layout at `layout`, at `sample_rate`.
    fn activate_at(plugin: &mut MxmFoldedSpring, layout: usize, sample_rate: f32) -> bool {
        plugin.activate(
            &MxmFoldedSpring::AUDIO_IO_LAYOUTS[layout],
            &BufferConfig {
                sample_rate,
                min_buffer_size: Some(1),
                max_buffer_size: 256,
                process_mode: ProcessMode::Realtime,
            },
            &mut TestActivateContext,
        )
    }

    /// **A rate that is not finite, or one below the floor, is refused**, and the refusal leaves
    /// the plugin as it was. Accepted, a NaN rate or one under about 44.4 Hz activated cleanly and
    /// then crossed the tank's `clamp` bounds on the first processed sample, on the audio thread.
    #[test]
    fn activation_refuses_a_non_finite_rate_and_any_below_the_floor() {
        for layout in 0..MxmFoldedSpring::AUDIO_IO_LAYOUTS.len() {
            for rate in [
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
                44.0,
                0.0,
                -48_000.0,
                MIN_SAMPLE_RATE.next_down(),
            ] {
                let mut refused = MxmFoldedSpring::default();
                assert!(
                    !activate_at(&mut refused, layout, rate),
                    "layout {layout} accepted {rate} Hz"
                );
                assert_eq!(
                    refused.sample_rate, 48_000.0,
                    "refusing {rate} Hz changed the plugin"
                );
            }
        }
    }

    /// **Every accepted rate is a promise**: the floor itself and the ordinary rates activate in
    /// both layouts and put a finite return from the tank on the input, with the loop open.
    #[test]
    fn the_rate_floor_and_the_ordinary_rates_activate_and_process() {
        for layout in 0..MxmFoldedSpring::AUDIO_IO_LAYOUTS.len() {
            let channels = MxmFoldedSpring::AUDIO_IO_LAYOUTS[layout]
                .main_input_channels
                .map_or(1, |c| c.get() as usize);
            for rate in [MIN_SAMPLE_RATE, 8_000.0, 44_100.0, 96_000.0, 192_000.0] {
                let mut plugin = MxmFoldedSpring::default();
                for (_, ptr, _) in plugin.params.param_map() {
                    // SAFETY: the call the wrapper makes before `activate`, at this rate.
                    unsafe { ptr._internal_update_smoother(rate, true) };
                }
                set_feedback(&plugin, 0.5);
                assert!(
                    activate_at(&mut plugin, layout, rate),
                    "layout {layout} refused {rate} Hz"
                );
                assert_eq!(plugin.sample_rate, rate);
                let input: Vec<f32> = (0..4_096).map(|i| (i as f32 * 0.01).sin() * 0.4).collect();
                let out = run(&mut plugin, &input, channels, 256);
                assert!(
                    out.iter().flatten().all(|s| s.is_finite()),
                    "layout {layout} at {rate} Hz is not finite"
                );
                assert!(
                    out.iter().all(|channel| *channel != input),
                    "layout {layout} at {rate} Hz added nothing to the input"
                );
            }
        }
    }

    #[test]
    fn the_tail_is_the_tanks_own() {
        // A longer tank owes a longer tail; a host that slept on the short one's figure would cut
        // the long one off.
        let short = SpringReverb::tail_samples_for(params::Tank::Short.model(), FS);
        let medium = SpringReverb::tail_samples_for(params::Tank::Medium.model(), FS);
        let long = SpringReverb::tail_samples_for(params::Tank::Long.model(), FS);
        assert!(short < medium && medium < long, "{short} {medium} {long}");
    }

    #[test]
    fn a_silent_input_ends_in_exact_silence() {
        let mut plugin = plugin(1);
        let mut input = impulse(48_000 * 8);
        input[0] = 1.0;
        let out = run(&mut plugin, &input, 1, 512);
        let tail = &out[0][out[0].len() - 4_800..];
        assert!(
            tail.iter().all(|s| *s == 0.0),
            "the tank must snap to exact zero rather than decay forever"
        );
    }
}

/// The plugin's name, checked where it escapes this crate.
#[cfg(test)]
mod identity {
    use super::{CLAP_ID, NAME};

    #[test]
    fn the_id_is_the_name_under_the_project_domain() {
        assert_eq!(CLAP_ID, format!("dk.mxm.{NAME}"));
    }

    /// `bundler.toml` names the same plugin this crate does — the one place the name is duplicated
    /// outside this crate, and nothing else would catch a disagreement.
    #[test]
    fn the_bundle_is_named_after_this_plugin() {
        mxm_plugin_test::bundle::is_named(env!("CARGO_MANIFEST_DIR"), env!("CARGO_PKG_NAME"), NAME);
    }
}

/// What a player reads — on hover in the editor, and in a host's plugin browser — speaks to the
/// player about the sound, never about the machine or the code (`mxm_plugin_test::hover_text`).
#[cfg(test)]
mod speaks_to_the_player {
    #[test]
    fn hover_text() {
        mxm_plugin_test::hover_text::speaks_to_the_player(env!("CARGO_MANIFEST_DIR"));
    }

    #[test]
    fn host_description() {
        mxm_plugin_test::hover_text::host_description_speaks_to_the_player(env!(
            "CARGO_MANIFEST_DIR"
        ));
    }
}
