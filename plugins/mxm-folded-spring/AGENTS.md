# AGENTS.md — plugins/mxm-folded-spring

Parent: [`../AGENTS.md`](../AGENTS.md)

# Purpose

The nice-plug shell for **mxm-folded-spring** under the parent’s *every built-in effect is also a
standalone effect* rule: the System-100 103 mixer’s spring reverb, with tank and feedback controls. Identity, the two parameters, the tank change,
presets, the two layouts and the wrapper's own rules — activity, Off, parking, the tail. The DSP is
[`crates/mxm-mono-00-dsp`](https://github.com/mxm-audio/mxm-mono-00/blob/main/crates/mxm-mono-00-dsp/AGENTS.md)'s `effects` module, **depended on
in place**; the collection plan is
`plans/plan-mxm-fx-collection.md` (`plans/plan-mxm-fx-collection.md` in the private archive).

Shared conventions — nice-plug's API, the preset rules, `process()` realtime rules, the editor
contract — live in the parent and are not restated here.


# Ownership

`Cargo.toml`, `LICENSE`, `README.md`, `control-map.json`, `presets/`, and `src/` — `lib.rs`,
`params.rs`, `preset.rs`, `telemetry.rs`, and `editor.rs` with `editor/{binding, sections}.rs`. The
brief it is built to is [`docs/briefs/mxm-folded-spring.md`](../../docs/briefs/mxm-folded-spring.md),
which is root-owned and gates the editor.

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

`SpringTankModel` was added to `mxm-mono-00-dsp` for this plugin, under the parent crate's rule that
an instrument's DSP may **gain inputs, never a behaviour change**. Three things hold that:

- `Medium` is the measured tank, is the default, and is spelled from the same constants the module
  always had rather than copied from them.
- Every delay line is sized for the **longest transit any tank asks for**, so `set_model` allocates
  nothing and is legal on the audio thread — and so the line length no longer depends on the tank,
  which is what keeps the read offsets identical.
- `crates/mxm-mono-00-dsp/tests/spring_tanks.rs` pins the instrument's render **by digest, taken on
  the revision before the tanks existed**, with a file that compiled against both. A failure there
  is not a licence to update the number.

`SpringReverb::tail_samples` still answers the measured tank's figure, because that is what the
instrument asks and the instrument has no other tank; this plugin asks `tail_samples_for`.

## Feedback is a path the tank did not have, and it is a declared departure

The 103's reverb is a send and a return; **Feedback** takes the tank's return and puts it back into
its own driver. That is not a fixed quantity opened, like the tank or the chorus's rate — it is a
signal path the box did not have, so it sits under the parent's *an instrument ships the effects
its original had* the way `mxm-mono-00`'s plug-out phaser and delay do: **by the owner's ruling**
(2026-09-04), recorded rather than assumed.

**The precedent, researched 2026-09-04 before it was built:**

- **Dub practice.** Routing a reverb's return back to its own send is the technique the music is
  built on, and a spring tank is what it was done to.
- **Doepfer A-199**, a three-spring Eurorack tank, ships a feedback control that feeds the reverb
  signal back to the input and self-oscillates at the top of its travel.
- **Music Thing Modular Spring Reverb Mk2** (2017) added feedback with a three-position invert
  switch, described by its author as anything from a subtle glow to screaming howlround — and
  deliberately left off CV, because it "normally seems to need a human ear and hand as part of the
  circuit".
- **Gamechanger Audio Light Pedal** sells the same thing as a mode: feedback "sets the spring tank
  into self-oscillation".

Sources: [Doepfer A-199](https://www.sweetwater.com/store/detail/A-199--doepfer-a-199-eurorack-spring-reverb-module),
[Music Thing Modular](https://www.musicthing.co.uk/Spring-Reverb/),
[Gamechanger Audio](https://gamechangeraudio.com/shop/light-pedal/),
[dub technique](https://blog.zzounds.com/2018/06/22/beat-techniques-dub-production/),
[the runaway question](https://www.modwiggler.com/forum/viewtopic.php?t=230338).

Four things make it work rather than merely exist:

- **The return is tapped before Level.** A send-and-return loop is not the mix knob: how loud the
  reverb sits is not how hard it drives itself.
- **Each lap is band-passed again**, because the fed-back signal re-enters through the tank's own
  input filter. That is faithful, and it is also the reported cure for spring feedback that runs
  away — here it is inherent rather than added.
- **A falling gain in the loop, not a clipper and not a limiter.** `x / (1 + (x/C)²)`: unity for
  small signals, and *decreasing* once the return is loud. Both of the obvious choices were tried
  and both were wrong in a way worth recording. A **clipper** squares the waveform off, and a
  square's harmonics are something the tank's own resonances amplify, so the loop's effective gain
  rises as it gets louder. A **limiter** pins the return's amplitude, which gives the oscillation a
  stable amplitude at *every* gain — it is how an AGC oscillator is built. `loop_shape`'s gain
  falls with level, so a sustained oscillation exists only above the threshold.
  `a_singing_loop_stays_finite` holds the bound.
- **Where it self-oscillates is measured, per tank, and it took two goes.** Self-oscillation here
  is the same thing a filter's resonance does: the loop sustains a tone with no input. The loop's
  gain is this control's **times the tank's own**, and the tanks differ by a factor of four, so a
  single constant put the threshold in a different place on each — the first build sang at about
  23 % of the travel on the short tank and never quite did on the long one (the owner heard 30 %).

  **There are two thresholds and calibrating to the wrong one is what cost three goes.** A tank
  sitting in silence has snapped its lines to exact zero, so a small signal is cleared before the
  loop can build on it: from silence it needs about twice the gain to start. A tank with anything
  in it needs only enough to keep what is there. Measured:

  | Tank | Holds, running | Starts, from silence |
  |---|---|---|
  | Short | 0.1368 | 0.2244 |
  | Medium | 0.2515 | 0.5325 |
  | Long | 0.1577 | 0.9663 |

  `SING_AT_*` are the **running** figures, because a tank somebody is playing through is never
  silent — that is the threshold a player meets, and calibrating to the other one is why the owner
  kept hearing it sing and stay on well below where the arithmetic said it began.

  The mapping then divides by `SINGS_AT_CONTROL` (**0.9**) and squares the travel below it
  ([`FEEDBACK_CURVE`]), so the two ends agree: above 0.9 an oscillation holds, below it one dies,
  and there is no band where turning the control down leaves it going.

  `the_hysteresis` is the facility that measured it and prints the levels rather than a verdict.
  `an_oscillation_holds_above_the_threshold_and_dies_below_it` holds the behaviour,
  `every_tank_starts_to_sing_at_the_same_point_on_the_control` holds the arithmetic for all three
  at no cost, and `the_two_thresholds_are_far_apart` holds the fact the calibration rests on.

  **Two of the measurements were wrong before they were right**, and both wrongs read as plausible
  results. `the_decay_against_gain` returned the ratio of two silences as `0.0 dB`, which looks
  exactly like a tone that is not decaying; half the printed curve was that. And the first
  hysteresis test asked whether the tail was above a floor, which after a loud start every setting
  is — the tank's own three-second decay is louder than any floor four seconds later. Ask whether
  it is *decaying or holding*, over windows far enough apart to tell.

  [`FEEDBACK_CURVE`]: crate::params::FEEDBACK_CURVE
- **The tail the plugin claims stretches by `1 / (1 - g)`**, clamped at `MAX_TAIL_S`. A host told
  the bare tank's figure would sleep the plugin while it was still ringing; a host told *forever*
  would never sleep it at all.

**Zero is the bare tank, to the bit.** `drive_clip(0.0)` is exactly zero, so at the default the
input drives the tank and nothing else — `feedback_at_zero_is_the_bare_tank_to_the_bit`. That is
what lets the three tank presets still be called the circuit, and
`only_the_regenerating_preset_feeds_the_tank_its_own_return` keeps them that way.

**No inversion control.** Music Thing's invert switch is real precedent and inverted feedback does
sound different, but one control is what was asked for and a second would need its own argument.
Recorded here so the next person reads a decision rather than an oversight.

## Level at zero is Off, and the core is parked

The parent's rule since `mxm-chorus-06`. At zero the tank is **emptied, not frozen**, preventing old audio from replaying when level returns;
from the next block `process` copies the input and returns. The park is left as soon as the
level moves or a tank change is pending.

## A non-finite sample never reaches the tank

The input pass that decides activity flushes a subnormal to zero and **zeroes a non-finite sample**,
which then counts as silence. Let through, a NaN sat in the band-pass and the springs, where the
quiet snap cannot see it, and the loop's memory handed it back every sample until Off. **The
feedback term is the tank's second way in** and is held to the same rule where it re-enters.
`a_non_finite_input_sample_is_silence_and_the_render_recovers` and
`a_non_finite_return_never_re_enters_the_tank` hold both. A parked block is a copy and inspects
nothing.

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

`activate` returns `false` for a non-finite sample rate or one below `MIN_SAMPLE_RATE` (50 Hz),
before it touches anything. `mxm-mono-00-dsp` works out the tank's corners every sample,
`f32::clamp`ed to `20 Hz ..= 0.45 × fs` for the dispersion all-passes; the bounds cross below about
44.4 Hz or at a NaN rate, which activated cleanly and then panicked on the audio thread. The floor
is derived, not musical, and no ceiling is set.
`activation_refuses_a_non_finite_rate_and_any_below_the_floor` and
`the_rate_floor_and_the_ordinary_rates_activate_and_process` hold both sides of it.

## Two controls, and why that still ships presets

`plans/plan-mxm-fx-collection.md` §2 ships the preset system **when a meaningful parameter exists
beyond level**, and expected this plugin to have none. The tank is one, so the rule turns the other
way: three tanks are three sounds. The factory set is one preset per tank at a level chosen for it,
generated from `preset.rs`'s `FACTORY_DESIGN` by the `#[ignore]`d `write_the_factory_presets`;
`every_factory_preset_selects_the_tank_it_is_named_after` is what catches a shifted enum.

## Editor paging

One indivisible Effects item, key 0: controls beside the tank display. The shared paging renderer
therefore hides the bar. `REFERENCE` and `MINIMUM` are the sizes, the minimum including
the controls/display floor and shell gutters (`the_minimum_window_holds_the_card_at_its_floor`). Indivisible overflow retains scrolling. Existing
painted-control, display and panel-fit tests remain; native/DPI QA is separate.

**The card is a `mxm_ui::tree`** (`crates/ui/AGENTS.md`, *A card body as data*).
`sections::card` describes the body once — Level and Feedback at the collection's knob column
(`control::knob_column`), the tank switch `SPACE_2` further on and beside them (label on the knobs' name line,
cells on their circles), then, `SPACE_5` beyond the row's spacing, the tank display — and that
description is measured for the card's floor and height and drawn leaf by leaf through the
bindings (`sections::paint`), through `paging::editor::show`. **The floor is computed**, the
tree's narrowest plus the card's chrome, with no usability minimum declared beside it, and the card
is exactly that wide (its ceiling is its floor, `plans/plan-editor-standard.md` A1). `REFERENCE` is
derived: the budget hugged around the card, held by `the_opening_size_is_the_budget_hugged`. The display states its own size: at least `TANK_MIN_WIDTH` wide,
filling the rest of the row, and as tall as the controls, at least `MIN_TANK_BOX`. `take_ring` is
read once, before the frame, and handed to the display's painting.
`the_minimum_window_holds_the_card_at_its_floor` holds the floor inside the minimum window, and
`every_card_passes_the_tree_checks_in_every_state` runs the shared checks
(`mxm_plugin_test::tree_checks`) at Init, Off, each tank, and ringing at the longest decay.

## No note port, so no developer channel

An effect declares `MidiConfig::None`, and the parent's developer channel rides on control changes.
The parent's *An effect carries none of it* says why that is a statement about effects rather than
an omission here.

## The control map claims the Effects page's last slot

`level` fills `fx.reverb`, which already existed for the built-in spring and is the same quantity.
`fx.reverb_tank` is **new**, stepped, three positions, appended into the Effects page's last free
slot in the same change as this plugin. **That page is now full**; the next effect that wants a role
needs a page, which is a decision about the controller's geography rather than a line in a file.

**`feedback` is deliberately unmapped**, and the page being full is only half the reason. The other
half is that it is the one control here that wants a hand on it — its own precedent says so — and a
role is a knob on a controller somebody may have assigned to something else. It is on the panel and
in every host's automation, which is where a control that can make the plugin sing belongs.

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

Commercial-host operation and hardware comparison remain manual gates.

# Child DOX Index

No child `AGENTS.md` files.
