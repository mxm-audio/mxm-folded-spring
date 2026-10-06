# NOTES.md — plugins/mxm-folded-spring/

The detail behind this folder's AGENTS.md: history, measurements, rationale and worked examples.
AGENTS.md is the contract; this file is the reference it links to.

## The tank is an addition to the instrument's DSP, and the instrument cannot feel it

`SpringTankModel` was added to `mxm-mono-00-dsp` for this plugin, under the parent crate's rule that
an instrument's DSP may **gain inputs, never a behaviour change**. Three things hold that:

- `Medium` is the measured tank, is the default, and is spelled from the same constants the module
  always had rather than copied from them.
- Every delay line is sized for the **longest transit any tank asks for**, so `set_model` allocates
  nothing and is legal on the audio thread — and so the line length no longer depends on the tank,
  which is what keeps the read offsets identical.
- `crates/mxm-mono-00-dsp/tests/spring_tanks.rs` (in the mxm-mono-00 repository) pins the
  instrument's render **by digest, taken on the revision before the tanks existed**, with a file
  that compiled against both. A failure there is not a licence to update the number.

`SpringReverb::tail_samples` still answers the measured tank's figure, because that is what the
instrument asks and the instrument has no other tank; this plugin asks `tail_samples_for`.

## Feedback is a path the tank did not have, and it is a declared departure

The 103's reverb is a send and a return; **Feedback** takes the tank's return and puts it back into
its own driver. That is not a fixed quantity opened, like the tank or the chorus's rate — it is a
signal path the box did not have, so it sits under the parent's *an instrument ships the effects
its original had* the way `mxm-mono-00`'s plug-out phaser and delay do: **by the owner's ruling**
(2026-09-04), recorded rather than assumed.

### The precedent, researched 2026-09-04 before it was built

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

### Four things make it work rather than merely exist

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
- **The tail the plugin claims stretches by `1 / (1 - g)`**, clamped at `MAX_TAIL_S`. A host told
  the bare tank's figure would sleep the plugin while it was still ringing; a host told *forever*
  would never sleep it at all.

### Two thresholds

**There are two thresholds and calibrating to the wrong one is what cost three goes.** A tank
sitting in silence has snapped its lines to exact zero, so a small signal is cleared before the loop
can build on it: from silence it needs about twice the gain to start. A tank with anything in it
needs only enough to keep what is there. Measured:

| Tank | Holds, running | Starts, from silence |
|---|---|---|
| Short | 0.1368 | 0.2244 |
| Medium | 0.2515 | 0.5325 |
| Long | 0.1577 | 0.9663 |

`SING_AT_*` are the **running** figures, because a tank somebody is playing through is never silent
— that is the threshold a player meets, and calibrating to the other one is why the owner kept
hearing it sing and stay on well below where the arithmetic said it began.

The mapping then divides by `SINGS_AT_CONTROL` (**0.9**) and squares the travel below it
([`FEEDBACK_CURVE`]), so the two ends agree: above 0.9 an oscillation holds, below it one dies, and
there is no band where turning the control down leaves it going.

[`FEEDBACK_CURVE`]: crate::params::FEEDBACK_CURVE

`the_hysteresis` is the facility that measured it and prints the levels rather than a verdict.
`an_oscillation_holds_above_the_threshold_and_dies_below_it` holds the behaviour,
`every_tank_starts_to_sing_at_the_same_point_on_the_control` holds the arithmetic for all three at
no cost, and `the_two_thresholds_are_far_apart` holds the fact the calibration rests on.

**Two of the measurements were wrong before they were right**, and both wrongs read as plausible
results. `the_decay_against_gain` returned the ratio of two silences as `0.0 dB`, which looks
exactly like a tone that is not decaying; half the printed curve was that. And the first hysteresis
test asked whether the tail was above a floor, which after a loud start every setting is — the
tank's own three-second decay is louder than any floor four seconds later. Ask whether it is
*decaying or holding*, over windows far enough apart to tell.

### Zero, and no inversion

**Zero is the bare tank, to the bit.** `drive_clip(0.0)` is exactly zero, so at the default the
input drives the tank and nothing else — `feedback_at_zero_is_the_bare_tank_to_the_bit`. That is
what lets the three tank presets still be called the circuit, and
`only_the_regenerating_preset_feeds_the_tank_its_own_return` keeps them that way.

**No inversion control.** Music Thing's invert switch is real precedent and inverted feedback does
sound different, but one control is what was asked for and a second would need its own argument.
Recorded here so the next person reads a decision rather than an oversight.

## A non-finite sample never reaches the tank

The input pass that decides activity flushes a subnormal to zero and **zeroes a non-finite sample**,
which then counts as silence. Let through, a NaN sat in the band-pass and the springs, where the
quiet snap cannot see it, and the loop's memory handed it back every sample until Off. **The
feedback term is the tank's second way in** and is held to the same rule where it re-enters.
`a_non_finite_input_sample_is_silence_and_the_render_recovers` and
`a_non_finite_return_never_re_enters_the_tank` hold both. A parked block is a copy and inspects
nothing.

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
therefore hides the bar. `REFERENCE` and `MINIMUM` are the sizes, the minimum including the
controls/display floor and shell gutters (`the_minimum_window_holds_the_card_at_its_floor`).
Indivisible overflow retains scrolling. Existing painted-control, display and panel-fit tests
remain; native/DPI QA is separate.

**The card is a `mxm_ui::tree`** (`crates/ui/AGENTS.md`, *A card body as data*; in mxm-kit, where
it is now [`crates/ui/NOTES.md`](https://github.com/mxm-audio/mxm-kit/blob/main/crates/ui/NOTES.md#a-card-body-as-data--tree)).
`sections::card` describes the body once — Level and Feedback at the collection's knob column
(`control::knob_column`), the tank switch `SPACE_2` further on and beside them (label on the knobs'
name line, cells on their circles), then, `SPACE_5` beyond the row's spacing, the tank display — and
that description is measured for the card's floor and height and drawn leaf by leaf through the
bindings (`sections::paint`), through `paging::editor::show`. **The floor is computed**, the tree's
narrowest plus the card's chrome, with no usability minimum declared beside it, and the card is
exactly that wide (its ceiling is its floor, `plans/plan-editor-standard.md` A1). `REFERENCE` is
derived: the budget hugged around the card, held by `the_opening_size_is_the_budget_hugged`. The
display states its own size: at least `TANK_MIN_WIDTH` wide, filling the rest of the row, and as
tall as the controls, at least `MIN_TANK_BOX`. `take_ring` is read once, before the frame, and handed
to the display's painting. `the_minimum_window_holds_the_card_at_its_floor` holds the floor inside
the minimum window, and `every_card_passes_the_tree_checks_in_every_state` runs the shared checks
(`mxm_plugin_test::tree_checks`) at Init, Off, each tank, and ringing at the longest decay.

## The control map claims the Effects page's last slot

`level` fills `fx.reverb`, which already existed for the built-in spring and is the same quantity.
`fx.reverb_tank` is **new**, stepped, three positions, appended into the Effects page's last free
slot in the same change as this plugin. **That page is now full**; the next effect that wants a role
needs a page, which is a decision about the controller's geography rather than a line in a file.

**`feedback` is deliberately unmapped**, and the page being full is only half the reason. The other
half is that it is the one control here that wants a hand on it — its own precedent says so — and a
role is a knob on a controller somebody may have assigned to something else. It is on the panel and
in every host's automation, which is where a control that can make the plugin sing belongs.
