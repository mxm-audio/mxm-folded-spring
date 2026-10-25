//! `mxm-folded-spring` through MXM Player's ordinary discovery, its parameter and state paths and
//! its serial effect chain, against the release bundle.
//!
//! The tank stays in `mxm-mono-00-dsp`, where it is measured. These hold the shipping boundary:
//! the bundle is classified as an effect, its control map loads and names parameters the bundle
//! has; the player reads every parameter at its default; a changed tank saved through the player
//! opens again in a fresh instance, by its permanent id; after `mxm-mono-01` the default spring is
//! finite and audible, adds nothing before its input, outlives it and snaps to exact silence; and
//! the player's bypass restores the dry render to the bit.

use mxm_player_harness::app_harness;
use mxm_player_harness::harness;

use harness::{CHANNELS, Harness};
use mxm_player::engine::Engine;
use mxm_player::events::input::Payload;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

const PLUGIN: &str = "mxm-folded-spring";
const SOURCE_ID: &str = "dk.mxm.mxm-mono-01";
const EFFECT_ID: &str = "dk.mxm.mxm-folded-spring";
const BLOCK: usize = 256;
// The note and its release, then the tank's own tail to exact silence, which the plugin's
// `a_silent_input_ends_in_exact_silence` holds within eight seconds of its input.
const BLOCKS: usize = 1_900;
/// Level, Tank and Feedback (`src/params.rs`).
const PARAMETERS: usize = 3;
/// The control map's roles, each with the parameter it names, by the name the host shows.
const ROLES: &[(&str, &str)] = &[("fx.reverb", "Level"), ("fx.reverb_tank", "Tank")];

/// This plugin's release bundle, or `None` when it has not been built: **the one skip**.
fn bundle() -> Option<PathBuf> {
    let found = mxm_player_harness::host_test::plugin_bundle(PLUGIN).map(|(_, file)| file);
    if found.is_none() {
        eprintln!("skipping: run `cargo xtask bundle {PLUGIN} --release`");
    }
    found
}

/// The source the chain plays, fetched beside the bundle (`test-bundles.txt`). Required once the
/// bundle exists, so a fetch that failed fails the tests rather than passing them unrun.
fn source() -> PathBuf {
    let path = mxm_player_harness::workspace_root().join("target/bundled/mxm-mono-01.clap");
    assert!(
        path.exists(),
        "{} is missing: run `cargo xtask fetch`",
        path.display()
    );
    path
}

/// The effect alone in a player engine, as the chain's Add puts it there.
fn engine(bundle: &Path) -> Engine {
    let mut engine = Engine::new();
    engine.add_fx(bundle, EFFECT_ID).expect("the effect loads");
    engine
}

#[test]
fn ordinary_discovery_classifies_the_bundle_and_its_map_names_real_parameters() {
    let Some(bundle) = bundle() else {
        return;
    };
    let directory = bundle.parent().unwrap().to_owned();
    let mut app = app_harness::AppHarness::new("folded-spring-discovery", vec![directory]);
    app.harness.state_mut().rescan();
    app.run();
    let state = app.state();
    let found = state
        .found
        .iter()
        .find(|found| found.id == EFFECT_ID)
        .unwrap_or_else(|| panic!("ordinary discovery did not find {EFFECT_ID}"));
    assert!(
        found.effect,
        "the one-input bundle was not classified as an effect"
    );
    assert!(
        app.app().control_map().knows_instrument(EFFECT_ID),
        "the control-map sidecar was not loaded"
    );

    let params = engine(&bundle).read_fx_params(0).expect("an effect");
    for (role, name) in ROLES {
        let parameter = params
            .params
            .iter()
            .find(|parameter| parameter.name == *name)
            .unwrap_or_else(|| panic!("no parameter called {name}"));
        assert_eq!(
            app.app().control_map().param_for(EFFECT_ID, role),
            Some(parameter.id),
            "{role}"
        );
    }
}

#[test]
fn the_player_reads_every_parameter_at_its_default() {
    let Some(bundle) = bundle() else {
        return;
    };
    let params = engine(&bundle).read_fx_params(0).expect("an effect");
    assert_eq!(params.params.len(), PARAMETERS);
    for parameter in &params.params {
        assert_eq!(parameter.value, parameter.default, "{}", parameter.name);
        assert!(!parameter.text.is_empty(), "{}", parameter.name);
        assert!(
            !parameter.is_hidden && !parameter.is_read_only,
            "{}",
            parameter.name
        );
    }
}

/// The nice-plug state the player saves: its length, then uncompressed JSON.
fn decode(bytes: &[u8]) -> Value {
    let size = u64::from_le_bytes(bytes[..8].try_into().expect("CLAP state length prefix"));
    assert_eq!(size as usize, bytes.len() - 8);
    serde_json::from_slice(&bytes[8..]).expect("uncompressed nice-plug state")
}

fn encode(value: &Value) -> Vec<u8> {
    let json = serde_json::to_vec(value).expect("state JSON");
    let mut bytes = Vec::with_capacity(json.len() + 8);
    bytes.extend_from_slice(&(json.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&json);
    bytes
}

/// **What the player saves, it opens.** The state it captures for a project, with the Long tank
/// written into it by its permanent id, loads into a fresh instance, which shows the tank and saves
/// it again unchanged.
#[test]
fn a_changed_tank_saves_and_opens_again_through_the_player() {
    let Some(bundle) = bundle() else {
        return;
    };
    let mut saved = engine(&bundle);
    let mut state = decode(&saved.capture_fx_state(0).expect("the state saves"));
    let params = state["params"].as_object_mut().expect("parameter map");
    assert_eq!(params.len(), PARAMETERS);
    assert_eq!(
        params["tank"],
        json!({"string": "medium"}),
        "the tank is not stored by its id, or Init is not Medium"
    );
    params.insert("tank".into(), json!({"string": "long"}));

    let mut opened = engine(&bundle);
    opened
        .load_fx_state(0, &encode(&state))
        .expect("the state opens");
    let params = opened.read_fx_params(0).expect("an effect");
    let tank = params
        .params
        .iter()
        .find(|parameter| parameter.name == "Tank")
        .expect("a Tank parameter");
    assert_eq!(tank.text, "Long");
    let again = decode(&opened.capture_fx_state(0).expect("the state saves again"));
    assert_eq!(again["params"], state["params"]);
}

fn render(with_effect: bool, bypassed: bool) -> Option<Vec<f32>> {
    let effect = bundle()?;
    let source = source();
    let chain: Vec<(&Path, &str)> = if with_effect {
        vec![(effect.as_path(), EFFECT_ID)]
    } else {
        vec![]
    };
    let mut harness = Harness::with_fx(&source, SOURCE_ID, 1, &chain).expect("the chain builds");
    if let Some(effect) = harness.fx.first() {
        effect.bypassed.store(bypassed, Ordering::Relaxed);
    }

    let mut output = Vec::with_capacity(BLOCKS * BLOCK * CHANNELS);
    for block in 0..BLOCKS {
        if block == 2 {
            assert!(harness.push_at(
                0,
                0,
                Payload::NoteOn {
                    channel: 0,
                    key: 60,
                    velocity: 100.0 / 127.0,
                },
            ));
        }
        if block == 28 {
            assert!(harness.push_at(
                0,
                0,
                Payload::NoteOff {
                    channel: 0,
                    key: 60,
                    velocity: 0.0,
                },
            ));
        }
        output.extend_from_slice(harness.render(BLOCK));
    }
    harness.shutdown();
    Some(output)
}

fn peak(interleaved: &[f32]) -> f32 {
    interleaved
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()))
}

fn first_sounding_frame(interleaved: &[f32]) -> Option<usize> {
    interleaved
        .chunks(CHANNELS)
        .position(|frame| frame.iter().any(|sample| *sample != 0.0))
}

fn last_sounding_frame(interleaved: &[f32]) -> Option<usize> {
    interleaved
        .chunks(CHANNELS)
        .enumerate()
        .rev()
        .find(|(_, frame)| frame.iter().any(|sample| *sample != 0.0))
        .map(|(frame, _)| frame)
}

#[test]
fn the_default_spring_is_finite_audible_and_snaps_to_exact_silence() {
    let Some(dry) = render(false, false) else {
        return;
    };
    let wet = render(true, false).expect("the bundle existed above");
    assert!(wet.iter().all(|sample| sample.is_finite()));
    assert_ne!(wet, dry, "the loaded effect must change the signal");

    let first = first_sounding_frame(&dry).expect("the source sounded");
    assert!(
        wet[..first * CHANNELS].iter().all(|sample| *sample == 0.0),
        "the effect produced sound before its input"
    );
    let dry_end = last_sounding_frame(&dry).expect("the source sounded");
    let wet_end = last_sounding_frame(&wet).expect("the effect sounded");
    assert!(
        wet_end > dry_end,
        "the spring ended at {wet_end}, the source at {dry_end}"
    );
    assert!(
        peak(&wet[dry_end * CHANNELS..]) > 1e-3,
        "the spring's tail after the source is inaudible"
    );
    let end = BLOCKS * BLOCK;
    assert!(
        wet_end < end - 4 * BLOCK,
        "the spring was still sounding at frame {wet_end} of {end}"
    );
}

#[test]
fn player_bypass_restores_dry_audio_to_the_bit() {
    let Some(dry) = render(false, false) else {
        return;
    };
    let bypassed = render(true, true).expect("the bundle existed above");
    assert!(peak(&dry) > 1e-3, "the source must be audible");
    assert_eq!(bypassed, dry);
}
