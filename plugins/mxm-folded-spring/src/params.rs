//! Parameter definitions.
//!
//! Every `#[id]` here is **permanent**. Changing one breaks every saved project that used the
//! plugin, so ids are part of the public interface.
//!
//! # Three controls: how much, which tank, and how much it feeds itself
//!
//! The 103's reverb is one knob, and that knob is [`level`](MxmFoldedSpringParams::level). The
//! second control is the one the hardware could not offer without a screwdriver: **which tank is
//! strung** (the owner's ruling, 2026-09-04). `Medium` is the measured tank — the one the
//! instrument has and the only one it can have — and `Short` and `Long` are chosen scalings of it,
//! which `crates/mxm-mono-00-dsp`'s `effects` module (in mxm-mono-00) states as such.
//!
//! The third is [`feedback`](MxmFoldedSpringParams::feedback), which is **not a quantity the circuit
//! fixed but a path it did not have**: the tank's return, sent back into its own driver. It starts
//! at zero, so the default patch is still exactly the tank. `lib.rs` carries the precedent and the
//! reasoning.
//!
//! There is no On: **Level at zero is Off**, the collection's rule since `mxm-chorus-06`, and at
//! zero the tank is emptied and not run at all.
//!
//! # Smoothing, and the one thing that cannot be smoothed
//!
//! Level multiplies into the audio and is smoothed here, per sample. **The tank cannot be**: its
//! springs are delay lines read at the tank's own transits, so changing tank means reading lines
//! written for different offsets. The DSP empties them on the change and the wrapper fades the wet
//! around it — `lib.rs`, *Changing tank*.

use mxm_mono_00_dsp::effects::SpringTankModel;
use mxm_preset::PresetIdentity;
use nice_plug::prelude::*;
use std::sync::{Arc, RwLock};

/// The wet level an inserted spring starts at.
///
/// **Chosen.** Nothing documents where the 103's knob sat, and an effect that starts silent fails
/// the collection's *an effect starts engaged* rule — it would read as broken. Enough to hear the
/// tank on the first note without burying the source.
pub const DEFAULT_LEVEL: f32 = 0.35;

/// Where on the control's travel the loop reaches the gain at which it diverges.
///
/// **The owner's number, 2026-09-04**, twice: the first build sang at about 30 %, and putting the
/// divergence point at 0.8 of a *straight* travel was still "far too soon". Both times the reason
/// was the same — an ear calls it singing well before the arithmetic does. A loop at three
/// quarters of its threshold is not diverging and is already a tone that will not go away.
///
/// So two things were changed together: divergence moved to 0.9, and the travel below it became
/// [`FEEDBACK_CURVE`] rather than a straight line, which is what actually keeps the audible
/// take-over out of the lower travel.
pub const SINGS_AT_CONTROL: f32 = 0.9;

/// How the control's travel maps onto the approach to that threshold.
///
/// **Squared**, so the bottom of the travel moves the loop gently and the last fifth does the
/// steep part. A straight line spends its middle in the region where the reverb has already taken
/// over: at 0.6 of a straight travel to a 0.8 threshold the loop is at three quarters of its
/// threshold, ringing on a pitch and going nowhere. Squared, the same 0.6 is at a third of it.
///
/// It is the same shape a hardware feedback control has for the same reason: the interesting range
/// is the last part, so the taper puts the travel there.
pub const FEEDBACK_CURVE: f32 = 2.0;

/// The raw loop gain at which a **running** oscillation holds, per tank, **measured** by
/// `the_hysteresis` in `lib.rs`.
///
/// # Why the running figure and not the starting one
///
/// There are two thresholds and they are far apart. A tank sitting in silence has already snapped
/// its lines to exact zero, so a small signal is cleared before the loop can build on it, and from
/// that state the loop needs about twice the gain to get going. A tank with something in it — any
/// tank in use, because somebody is playing through it — needs only enough gain to keep what is
/// there, and that is this number.
///
/// Calibrating to the starting figure is what made the control wrong twice: the owner heard it
/// singing and staying on well below where the arithmetic said it should begin, because what they
/// were doing was playing into it rather than sweeping a silent one. Calibrating to the running
/// figure makes the two ends agree — above [`SINGS_AT_CONTROL`] an oscillation holds, below it one
/// dies, and there is no band where turning the control back down leaves it going.
///
/// It is not one number because it is not one tank: each tank's own return has its own gain.
pub const fn sing_at(tank: Tank) -> f32 {
    match tank {
        Tank::Short => SING_AT_SHORT,
        Tank::Medium => SING_AT_MEDIUM,
        Tank::Long => SING_AT_LONG,
    }
}

/// Measured 2026-09-04 by `the_hysteresis`, by binary search on the gain at which an oscillation
/// started at the top of the control still holds twelve seconds after the control came down.
pub const SING_AT_SHORT: f32 = 0.1368;
pub const SING_AT_MEDIUM: f32 = 0.2515;
pub const SING_AT_LONG: f32 = 0.1577;

/// The same tanks' **starting** thresholds from a silent tank, measured by
/// `the_oscillation_thresholds`. Roughly twice the running figures above, and for the long tank
/// six times, because a silent tank has snapped its lines to zero and clears a small signal before
/// the loop can build on it.
///
/// A record rather than a number in use — a tank somebody is playing through is never silent — and
/// `the_two_thresholds_are_far_apart` is what keeps the record honest.
#[cfg(test)]
pub const START_FROM_SILENCE: [f32; 3] = [0.2244, 0.5325, 0.9663];

/// The tanks, as a parameter.
///
/// A mirror of [`SpringTankModel`] rather than the DSP type itself: `#[id]` and `#[name]` are this
/// plugin's public interface — a saved project stores the id string — and the DSP crate must not
/// have to care what a host writes into a file. [`Tank::model`] is the only crossing, and
/// `every_tank_maps_to_its_own_model` holds it.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tank {
    #[id = "short"]
    #[name = "Short"]
    Short,
    #[id = "medium"]
    #[name = "Medium"]
    Medium,
    #[id = "long"]
    #[name = "Long"]
    Long,
}

impl Tank {
    pub const ALL: [Self; 3] = [Self::Short, Self::Medium, Self::Long];

    pub const fn model(self) -> SpringTankModel {
        match self {
            Self::Short => SpringTankModel::Short,
            Self::Medium => SpringTankModel::Medium,
            Self::Long => SpringTankModel::Long,
        }
    }
}

/// Formats a parameter value for display.
type ValueToString = Arc<dyn Fn(f32) -> String + Send + Sync>;
/// Parses a typed-in value, returning `None` if it cannot be understood.
type StringToValue = Arc<dyn Fn(&str) -> Option<f32> + Send + Sync>;

fn v2s_percent() -> ValueToString {
    Arc::new(|v| format!("{:.0} %", v * 100.0))
}

fn s2v_percent() -> StringToValue {
    Arc::new(|text| {
        text.trim()
            .trim_end_matches('%')
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| v / 100.0)
    })
}

#[derive(Params)]
pub struct MxmFoldedSpringParams {
    /// The wet level. **Zero is Off.**
    #[id = "level"]
    pub level: FloatParam,
    /// Which tank is strung.
    #[id = "tank"]
    pub tank: EnumParam<Tank>,
    /// How much of the tank's return goes back into its driver. **Zero is the bare tank.**
    #[id = "feedback"]
    pub feedback: FloatParam,

    /// Which preset is loaded, and what it looked like when it was.
    ///
    /// **Persisted with the patch, not beside it.** nice-plug carries non-parameter state through
    /// the `Params` derive's `#[persist]`, so it belongs here rather than as a field on the plugin
    /// struct. The shape is `mxm-preset`'s, shared with every instrument and effect.
    #[persist = "preset"]
    pub preset: RwLock<PresetIdentity>,
}

impl Default for MxmFoldedSpringParams {
    /// The init patch, and the set of CLAP `default_value`s: **the measured tank, engaged.** An
    /// effects exception to the instruments' *every amount starts at zero* — an inserted effect
    /// demonstrates the effect it is named after.
    fn default() -> Self {
        Self {
            level: FloatParam::new(
                "Level",
                DEFAULT_LEVEL,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_value_to_string(v2s_percent())
            .with_string_to_value(s2v_percent()),
            tank: EnumParam::new("Tank", Tank::Medium),
            // **Zero**, unlike Level: this is a path the hardware did not have, and the
            // instruments' *every amount starts at zero* rule is the right one for a control that
            // is an addition rather than something the circuit fixed. It also keeps the default
            // patch, and every factory preset that leaves it alone, exactly the tank.
            feedback: FloatParam::new("Feedback", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(v2s_percent())
                .with_string_to_value(s2v_percent()),

            preset: RwLock::new(PresetIdentity::none()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each tank names a different model, and every model is named by a tank. A mirror that lost a
    /// variant would silently map two ids onto one sound.
    #[test]
    fn every_tank_maps_to_its_own_model() {
        let mapped: Vec<SpringTankModel> = Tank::ALL.iter().map(|t| t.model()).collect();
        for (index, model) in mapped.iter().enumerate() {
            assert!(
                !mapped[index + 1..].contains(model),
                "two tanks map to {model:?}"
            );
        }
        for model in SpringTankModel::ALL {
            assert!(
                mapped.contains(&model),
                "{model:?} is a tank the DSP has and this plugin cannot select"
            );
        }
    }

    /// The default patch is the measured tank, engaged and audible.
    #[test]
    fn the_default_is_the_measured_tank_and_it_is_engaged() {
        let params = MxmFoldedSpringParams::default();
        assert_eq!(params.tank.value(), Tank::Medium);
        assert_eq!(params.tank.value().model(), SpringTankModel::Medium);
        assert!(
            params.level.value() > 0.0,
            "an inserted effect demonstrates itself"
        );
        assert_eq!(
            params.feedback.value(),
            0.0,
            "the default patch is the bare tank, with none of the path the circuit did not have"
        );
    }
}
