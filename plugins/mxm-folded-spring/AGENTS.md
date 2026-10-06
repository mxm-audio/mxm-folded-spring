# AGENTS.md — plugins/mxm-folded-spring

Parent: [`../AGENTS.md`](../AGENTS.md) · The measurements, precedent and reasoning behind each rule:
[`NOTES.md`](NOTES.md)

# Purpose

The nice-plug shell for **mxm-folded-spring** under the parent’s *every built-in effect is also a
standalone effect* rule: the System-100 103 mixer’s spring reverb, with tank and feedback controls.
Identity, the two parameters, the tank change, presets, the two layouts and the wrapper's own rules
— activity, Off, parking, the tail. The DSP is
[`crates/mxm-mono-00-dsp`](https://github.com/mxm-audio/mxm-mono-00/blob/main/crates/mxm-mono-00-dsp/AGENTS.md)'s
`effects` module, **depended on in place**; the collection plan is `plans/plan-mxm-fx-collection.md`
in the private archive.

Shared conventions — nice-plug's API, the preset rules, `process()` realtime rules, the editor
contract — live in the parent and are not restated here.

# Ownership

`Cargo.toml`, `README.md`, `control-map.json`, `presets/`, and `src/` — `lib.rs`, `params.rs`,
`preset.rs`, `telemetry.rs`, and `editor.rs` with `editor/{binding, sections}.rs`; the licence is the
workspace's (`../../LICENSE`). The brief it is built to is
[`docs/briefs/mxm-folded-spring.md`](../../docs/briefs/mxm-folded-spring.md), which is root-owned and
gates the editor.

# Local Contracts

## Permanent identifiers

| What | Value |
|---|---|
| CLAP id | `dk.mxm.mxm-folded-spring` |
| Display name | `mxm-folded-spring` |
| Parameter ids | `level`, `tank`, `feedback` |
| Tank ids | `short`, `medium`, `long` |

Changing any of these breaks every saved project that used the plugin. The id is assembled from
`plugin_name!` so a directory rename cannot silently change it, and
`the_bundle_is_named_after_this_plugin` holds `bundler.toml` to the same name.

## The tank is an addition to the instrument's DSP, and the instrument cannot feel it

- `SpringTankModel` was added to `mxm-mono-00-dsp` under that crate's rule that an instrument's DSP
  may **gain inputs, never a behaviour change**. `Medium` is the measured tank and the default.
- Every delay line is sized for the **longest transit any tank asks for**, so `set_model` allocates
  nothing and is legal on the audio thread.
- `spring_tanks.rs` in mxm-mono-00 pins the instrument's render by digest, taken before the tanks
  existed. **A failure there is not a licence to update the number.**
- This plugin asks `tail_samples_for`; `SpringReverb::tail_samples` keeps the instrument's figure
  ([NOTES.md § The tank](NOTES.md#the-tank-is-an-addition-to-the-instruments-dsp-and-the-instrument-cannot-feel-it)).

## Feedback is a path the tank did not have, and it is a declared departure

Feedback puts the tank's return back into its own driver: a signal path the 103 did not have,
shipped **by the owner's ruling** (2026-09-04). Precedent, sources and measurements:
[NOTES.md § Feedback](NOTES.md#feedback-is-a-path-the-tank-did-not-have-and-it-is-a-declared-departure).

- **The return is tapped before Level.** Each lap re-enters through the tank's own input band-pass.
- **A falling gain in the loop (`loop_shape`), not a clipper and not a limiter**, so a sustained
  oscillation exists only above the threshold. `a_singing_loop_stays_finite` holds the bound.
- The threshold is measured per tank and calibrated to the **running** figures (`SING_AT_*`), never
  the from-silence ones. The mapping divides by `SINGS_AT_CONTROL` and squares the travel below it,
  so above it an oscillation holds and below it one dies, with no band where turning the control
  down leaves it going. `an_oscillation_holds_above_the_threshold_and_dies_below_it`,
  `every_tank_starts_to_sing_at_the_same_point_on_the_control` and
  `the_two_thresholds_are_far_apart` hold it.
- When measuring, ask whether the tail is *decaying or holding*, over windows far enough apart to
  tell — never whether it is above a floor
  ([NOTES.md § Two thresholds](NOTES.md#two-thresholds)).
- **The tail the plugin claims stretches by `1 / (1 - g)`**, clamped at `MAX_TAIL_S`.
- **Zero is the bare tank, to the bit** (`feedback_at_zero_is_the_bare_tank_to_the_bit`), and only
  the regenerating preset feeds the tank its own return
  (`only_the_regenerating_preset_feeds_the_tank_its_own_return`).
- **No inversion control** — a decision, not an oversight
  ([NOTES.md § Zero, and no inversion](NOTES.md#zero-and-no-inversion)).

## Level at zero is Off, and the core is parked

The parent's rule since `mxm-chorus-06`. At zero the tank is **emptied, not frozen**, preventing old
audio from replaying when level returns; from the next block `process` copies the input and returns.
The park is left as soon as the level moves or a tank change is pending.

## A non-finite sample never reaches the tank

The activity pass flushes a subnormal to zero and **zeroes a non-finite sample**, which then counts
as silence. **The feedback term is the tank's second way in** and is held to the same rule where it
re-enters. `a_non_finite_input_sample_is_silence_and_the_render_recovers` and
`a_non_finite_return_never_re_enters_the_tank` hold both. A parked block is a copy and inspects
nothing ([NOTES.md § A non-finite sample](NOTES.md#a-non-finite-sample-never-reaches-the-tank)).

## Changing tank is a fade, not a crossfade

A tank cannot be crossfaded into another: its springs are delay lines read at its own transits, so
the lines hold a return computed for offsets that no longer apply. The wrapper fades the **wet**
out over `TANK_SWITCH_S`, swaps at silence, and fades back in; the dry is untouched throughout.
`changing_tank_never_steps_the_output` holds it by measuring the largest sample-to-sample step
across a change.

**A tank asked for during a fade is simply the new destination** — the fade does not restart, so
spinning the switch cannot leave the wet stuck at zero.

## Two layouts, and what stereo means for a mono tank

Mono in, mono out is the tank and the default. Stereo in, stereo out sums the two channels into the
one tank and adds its return to **both**, each side's dry untouched. So the reverb is mono and the
source keeps its image, which is what a mono tank across a stereo desk does — and unlike the
chorus, no dry-leg crossfade is needed, because the dry is never replaced.

## Activation refuses a rate the tank cannot run at

`activate` returns `false` for a non-finite sample rate or one below `MIN_SAMPLE_RATE`, before it
touches anything; the floor is derived from where the DSP's clamp bounds cross, not musical, and no
ceiling is set. `activation_refuses_a_non_finite_rate_and_any_below_the_floor` and
`the_rate_floor_and_the_ordinary_rates_activate_and_process` hold both sides
([NOTES.md § Activation](NOTES.md#activation-refuses-a-rate-the-tank-cannot-run-at)).

## Presets

One factory preset per tank, generated from `preset.rs`'s `FACTORY_DESIGN` by the `#[ignore]`d
`write_the_factory_presets`; `every_factory_preset_selects_the_tank_it_is_named_after` catches a
shifted enum ([NOTES.md § Two controls](NOTES.md#two-controls-and-why-that-still-ships-presets)).

## Editor

- One indivisible Effects item, key 0: controls beside the tank display; the paging bar is hidden.
  Indivisible overflow retains scrolling.
- The card is a `mxm_ui::tree`; **the floor is computed**, never declared, and the card is exactly
  that wide. `REFERENCE` (`the_opening_size_is_the_budget_hugged`) and `MINIMUM`
  (`the_minimum_window_holds_the_card_at_its_floor`) are held by tests;
  `every_card_passes_the_tree_checks_in_every_state` runs the shared checks. `take_ring` is read
  once, before the frame ([NOTES.md § Editor paging](NOTES.md#editor-paging)).

## No note port, so no developer channel

An effect declares `MidiConfig::None`, and the parent's developer channel rides on control changes.
The parent's *An effect carries none of it* says why that is a statement about effects rather than
an omission here.

## The control map claims the Effects page's last slot

`level` fills `fx.reverb`; `fx.reverb_tank` is new, stepped, three positions, in the Effects page's
last free slot. **That page is now full**: the next effect that wants a role needs a page.
**`feedback` is deliberately unmapped**
([NOTES.md § The control map](NOTES.md#the-control-map-claims-the-effects-pages-last-slot)).

# Work Guidance

- **The tanks' numbers live in the DSP crate, not here.** `params::Tank` is a mirror that exists so
  `#[id]` strings — which a saved project stores — are this plugin's business and not the DSP's.
  `every_tank_maps_to_its_own_model` holds the two lists together.
- Adding a tank means adding a `SpringTankModel` variant, a `Tank` variant, a switch cell and a
  factory preset. The tests name each of those if one is forgotten.
- A tank longer than `LONGEST_TRANSIT_MS` needs that constant raised, or its springs will be
  clamped to the line length. `every_springs_transit_fits_the_displays_scale` catches the display
  half; the DSP half is the clamp in `Spring::process`.

# Verification

```bash
cargo test -p mxm-folded-spring
cargo test -p mxm-mono-00-dsp --test spring_tanks   # the instrument's render, pinned
cargo clippy -p mxm-folded-spring --all-targets
# The panel, light and dark, for review -> target/layout-tree/mxm-folded-spring/<MXM_PICTURES tag>/
MXM_PICTURES=after cargo test -p mxm-folded-spring --lib tree_pictures -- --ignored
cargo fmt -p mxm-folded-spring --check
cargo xtask bundle mxm-folded-spring --release
clap-validator validate "target/bundled/mxm-folded-spring.clap"
```

`mxm-mono-00-dsp` and its `spring_tanks` test live in the mxm-mono-00 repository; this one takes the
crate by git tag.

Commercial-host operation and hardware comparison remain manual gates.

# Child DOX Index

No child `AGENTS.md` files.
