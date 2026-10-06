//! Presets: this plugin's factory set, and what the collection's preset crate needs of it.
//!
//! The format, the library on disk, favourites, the loaded identity and the app-bar controls are
//! `mxm-preset`'s. What is left here is what only this plugin knows: its id, its parameters, and
//! its three sounds.
//!
//! # Why an effect this small ships presets at all
//!
//! `plans/plan-mxm-fx-collection.md` §2 ships the preset system **when a meaningful parameter
//! exists beyond level**, and expected this plugin to have none. The tank control is one, so the
//! rule turns the other way: three tanks are three sounds, and a sound with a name is a preset.
//! Each factory preset is one tank at a level chosen for it — a short tank takes more level before
//! it is heard, a long one less before it swamps the source.

use std::sync::RwLock;

pub use mxm_preset::{
    Category, Entry, INIT_NAME, Library, Loaded, Origin, Preset, PresetIdentity, Refused, Value,
    factory, loaded, mark_loaded, mark_none, read_favourites, snapshot, write_favourites,
};

use crate::params::MxmFoldedSpringParams;

impl mxm_preset::Instrument for MxmFoldedSpringParams {
    fn clap_id(&self) -> &'static str {
        crate::CLAP_ID
    }

    /// In declaration order, from the one list the editor draws from.
    fn parameters(&self) -> Vec<(&'static str, &dyn mxm_preset::ErasedParam)> {
        crate::editor::sections::all_parameters(self)
            .into_iter()
            .map(|bound| (bound.id, bound.param))
            .collect()
    }

    fn identity(&self) -> &RwLock<PresetIdentity> {
        &self.preset
    }

    fn factory_files(&self) -> &'static [(&'static str, &'static str)] {
        FACTORY_FILES
    }
}

/// The factory set, compiled in: **one per tank**, and Init is not one of them.
pub const FACTORY_FILES: &[(&str, &str)] = &[
    ("Short tank", include_str!("../presets/short-tank.json")),
    ("Medium tank", include_str!("../presets/medium-tank.json")),
    ("Long tank", include_str!("../presets/long-tank.json")),
    ("Regenerating", include_str!("../presets/regenerating.json")),
];

#[cfg(test)]
mod tests {
    use super::*;
    use mxm_preset::user_root;
    use nice_plug::params::Param;

    fn params() -> MxmFoldedSpringParams {
        MxmFoldedSpringParams::default()
    }

    /// Prints every parameter as eleven `normalised=formatted` steps, for preset design.
    ///
    /// ```text
    /// cargo test -p mxm-folded-spring --lib the_mapping_table -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "prints what each normalised value means, for preset design"]
    fn the_mapping_table() {
        let params = params();
        for bound in crate::editor::sections::all_parameters(&params) {
            let steps: Vec<String> = (0..=10)
                .map(|i| {
                    let v = i as f32 / 10.0;
                    format!("{v:.1}={}", bound.param.format(v))
                })
                .collect();
            eprintln!("{:<12} {}", bound.id, steps.join("  "));
        }
    }

    /// One designed preset: its name, its category, and the normalised values that make it.
    type Design = (&'static str, Category, &'static [(&'static str, f32)]);

    /// The four sounds: one per tank at a level chosen for it, and one that uses the path the
    /// tank did not have.
    ///
    /// The tank is written as its normalised position — `0`, `0.5`, `1` for three values — because
    /// that is what a file stores. `every_factory_preset_selects_the_tank_it_is_named_after` reads
    /// it back as a tank, so a shifted enum could not pass quietly.
    const FACTORY_DESIGN: &[Design] = &[
        // A short tank is quiet and splashy, so it takes more level before it is heard.
        ("Short tank", Category::Fx, &[("tank", 0.0), ("level", 0.5)]),
        (
            "Medium tank",
            Category::Fx,
            &[("tank", 0.5), ("level", 0.35)],
        ),
        // A long tank swamps a source at the same level, so it gets less.
        ("Long tank", Category::Fx, &[("tank", 1.0), ("level", 0.28)]),
        // **What the feedback path is for**: the long tank driving itself, which is the dub send
        // routed back to its own return. Short of the setting where it sings, and quieter again,
        // because a tank feeding itself gets loud on its own.
        (
            "Regenerating",
            Category::Fx,
            &[("tank", 1.0), ("level", 0.22), ("feedback", 0.55)],
        ),
    ];

    /// Writes the three factory presets to `plugins/mxm-folded-spring/presets/`.
    ///
    /// A facility, not a test — and the *only* thing that writes those files.
    ///
    /// ```text
    /// cargo test -p mxm-folded-spring --lib write_the_factory_presets -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "writes the factory preset files"]
    fn write_the_factory_presets() {
        let params = params();
        for (name, category, overrides) in FACTORY_DESIGN {
            let preset = generated(&params, name, *category, overrides);
            let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("presets")
                .join(format!("{}.json", name.to_lowercase().replace(' ', "-")));
            std::fs::write(&file, preset.to_json()).expect("write the preset");
            eprintln!("wrote {}", file.display());
        }
    }

    /// What the generator would write for one design, in memory.
    fn generated(
        params: &MxmFoldedSpringParams,
        name: &str,
        category: Category,
        overrides: &[(&str, f32)],
    ) -> Preset {
        let bindings = crate::editor::sections::all_parameters(params);
        let mut preset = Preset::init(params);
        preset.name = name.to_owned();
        preset.category = category;
        for (id, v) in overrides {
            let bound = bindings
                .iter()
                .find(|bound| bound.id == *id)
                .unwrap_or_else(|| panic!("{name:?} names `{id}`, which is not a parameter"));
            preset.params.insert(
                (*id).to_owned(),
                Value {
                    v: *v,
                    text: bound.param.format(*v),
                },
            );
        }
        preset
    }

    #[test]
    fn every_designed_preset_names_real_parameters() {
        let params = params();
        let bindings = crate::editor::sections::all_parameters(&params);
        for (name, _category, overrides) in FACTORY_DESIGN {
            for (id, v) in *overrides {
                assert!(
                    bindings.iter().any(|bound| bound.id == *id),
                    "{name:?} names `{id}`, which is not a parameter of this plugin"
                );
                assert!(
                    (0.0..=1.0).contains(v),
                    "{name:?} sets `{id}` to {v}, which is not a normalised value"
                );
            }
        }
    }

    #[test]
    fn the_factory_files_match_the_design_they_were_generated_from() {
        // The generator is `#[ignore]`d, so nothing forces it to have been run. This is what says
        // the shipped files are the current design rather than a stale one.
        let params = params();
        for (name, category, overrides) in FACTORY_DESIGN {
            let (_, text) = FACTORY_FILES
                .iter()
                .find(|(file_name, _)| file_name == name)
                .unwrap_or_else(|| panic!("no factory file for {name:?}"));
            let shipped = Preset::parse(text, crate::CLAP_ID).expect("parses");
            let expected = generated(&params, name, *category, overrides);
            assert_eq!(
                shipped, expected,
                "{name:?} on disk is not what the design generates — regenerate the files"
            );
        }
    }

    /// Each preset really strings the tank its name claims. A shifted enum would otherwise leave
    /// three files whose names and sounds had quietly parted company.
    #[test]
    fn every_factory_preset_selects_the_tank_it_is_named_after() {
        use crate::params::Tank;
        let params = params();
        for (name, text, want) in [
            ("Short tank", FACTORY_FILES[0].1, Tank::Short),
            ("Medium tank", FACTORY_FILES[1].1, Tank::Medium),
            ("Long tank", FACTORY_FILES[2].1, Tank::Long),
            ("Regenerating", FACTORY_FILES[3].1, Tank::Long),
        ] {
            let preset = Preset::parse(text, crate::CLAP_ID).expect("parses");
            let v = preset.params["tank"].v;
            assert_eq!(params.tank.preview_plain(v), want, "{name}");
            assert!(
                preset.params["level"].v > 0.0,
                "{name} is silent, so it demonstrates nothing"
            );
        }
    }

    /// **Only the preset that is about feedback uses it.** The other three are the tank alone,
    /// which is what lets each of them still be called the circuit.
    #[test]
    fn only_the_regenerating_preset_feeds_the_tank_its_own_return() {
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID).expect("parses");
            let feedback = preset.params["feedback"].v;
            if *name == "Regenerating" {
                assert!(feedback > 0.0, "{name} is the one that uses the path");
            } else {
                assert_eq!(feedback, 0.0, "{name} must be the bare tank");
            }
        }
    }

    #[test]
    fn the_user_root_is_under_this_plugins_own_id() {
        let Some(root) = user_root(crate::CLAP_ID) else {
            return;
        };
        assert!(root.ends_with("presets"));
        assert!(root.to_string_lossy().contains(crate::CLAP_ID));
    }

    #[test]
    fn no_two_factory_presets_are_the_same_sound() {
        for (index, (name, text)) in FACTORY_FILES.iter().enumerate() {
            let a = Preset::parse(text, crate::CLAP_ID).expect("parses");
            for (other, text) in &FACTORY_FILES[index + 1..] {
                let b = Preset::parse(text, crate::CLAP_ID).expect("parses");
                assert_ne!(a.params, b.params, "{name:?} and {other:?} are identical");
            }
        }
    }

    #[test]
    fn every_factory_preset_has_a_category() {
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID).expect("parses");
            assert_ne!(
                preset.category,
                Category::Uncategorised,
                "factory preset {name:?} has no category"
            );
        }
    }

    #[test]
    fn every_factory_preset_parses_and_is_for_this_instrument() {
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID)
                .unwrap_or_else(|e| panic!("factory preset {name:?} does not parse: {e}"));
            assert_eq!(preset.name, *name, "the file's name must match its listing");
        }
    }

    #[test]
    fn every_factory_preset_covers_every_parameter() {
        let params = params();
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID).expect("parses");
            let (_, problems) = preset.resolve(&params);
            assert!(
                problems.is_empty(),
                "factory preset {name:?} is incomplete: {problems:?}"
            );
        }
    }

    #[test]
    fn the_factory_list_begins_with_init() {
        let params = params();
        let all = factory(&params);
        assert_eq!(all[0].name, INIT_NAME);
        assert_eq!(all.len(), FACTORY_FILES.len() + 1);
    }
}
