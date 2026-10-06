# mxm-folded-spring — UI design brief

Required by mxm-kit's [`MXM_DESIGN_SYSTEM.md`](https://github.com/mxm-audio/mxm-kit/blob/main/docs/MXM_DESIGN_SYSTEM.md) §14. Answers the ten questions in order, then records the
deliberate deviations.

**Plugin:** the Roland System-100 103 mixer's spring reverb as a standalone effect, with the tank
itself as a control. Audio in, audio out; no notes.

**Written alongside the panel rather than before it, and that is a deviation worth naming.** The
gate exists so an editor's shape is decided rather than discovered. What decided this one is
`docs/briefs/mxm-chorus-06.md`, which settled *what an effect's editor is* in this collection — one
view, no view bar, the collection accent, the display beside the controls, and (since R2 of
`plans/plan-editor-standard.md`) a window its card's width, the app bar compacting to it —
and this plugin inherits every one of those with no new argument. What is genuinely new here is the
display (§8) and the tank switch (§2), and those were designed here first.

---

## 1. Primary sound-design task

**Deciding how far away the source should sound, and in what size of room.** A spring is not a room,
which is the point: it has a character — the "boing", the dispersion that makes highs arrive late —
and the task is choosing how much of that character to add and how long it should ring.

The hardware offered the first half of that and not the second: one knob, one tank, bolted in. The
second control here is the one you would have needed a screwdriver for.

## 2. The three to five parameters users reach for most

Three:

1. **Level** — how much tank. Performed, and the on/off: at zero the reverb is off.
2. **Feedback** — how hard the tank drives itself. Performed, and the one that can run away.
3. **Tank** — which springs. Short, Medium or Long.

**Level and Feedback both take Primary sizing**, one row of one diameter so their values sit on one
line, and the tank is a segmented switch beside them on the knobs' own grid.
A switch rather than a knob because the tanks are three things and not a range: a continuous control
between two tanks would have to mean crossfading them, which is not a thing a spring does.

## 3. Signal flow that must be visible

```
in ──┬───────────── dry ─────────────┬─► out
     │                               │
     └─► band-pass ─┬─► spring 1 ──┐ │
                    ├─► spring 2 ──┼─┴─ × Level
                    └─► spring 3 ──┘
                        (a tank; Short has two)
```

Two facts, and the display carries both:

- **A tank is springs of different lengths.** The three do not agree on a delay, which is why a
  spring reverb sounds like a spread rather than an echo — and why the tanks differ audibly at all.
- **The dry is untouched.** Level adds; it never trades the source away.

## 4. Which controls belong in Play view

**Not applicable — no `Play` view.** See §6.

## 5. Advanced controls and their disclosure

**None.** Two controls do not divide into primary and advanced.

**Consequence for the developer channel**: it arrives as MIDI CC and an effect has no note port, so
this plugin carries none of it — `plugins/AGENTS.md` states that for effects generally.

## 6. Views

**One indivisible Effects card**, the singleton case of design-system §3.2's dynamic paging.
No bar or duplicate Parameters list. Controls and tank display stay together; an oversized card
retains scrolling. There is no developer MIDI channel because this effect has no note port.

## 7. Identity accent

**The collection's, unchanged**, and for the reason the chorus's brief gives: §5.3's identity hues
tell a *rack of instruments* apart, an effect is told apart by its name in a chain, and a second
accent scheme would have nothing keeping it from colliding with the instruments'.

## 8. Live visualizations

**One: the tank.** Each spring is a line as long as its own transit, drawn against the longest any
tank asks for, so:

- **the tanks are visibly different** — three lengths, and Short is visibly two springs rather than
  three;
- **a spring's length is a length**, not a percentage, because the scale does not rescale itself to
  the tank in force.

The lines brighten with `Telemetry::take_ring` — **the wet the audio thread actually added**, after
the level and the tank-change fade. So a tank that has snapped to silence goes dark, which the Level
knob cannot tell you, and a tank swapped mid-tail visibly dims through the swap. The geometry comes
from the DSP's own `SpringTank`, so a display and a sound that disagreed would be a compile error
rather than a drawing mistake.

A **level meter** in the app bar, as every other editor has: a wet added on top of a full-scale dry
can clip, and this is the only place that says so.

## 9. What is removed from the source hardware layout, and why

The source layout is **one knob on a mixer channel strip**.

- **Nothing is removed.** The knob is Level.
- **Two things are added.** The **tank**, because the hardware's was a soldered-in assembly and a
  plugin's is a choice; refusing to offer it would be faithfulness to a limitation rather than to a
  sound. And **Feedback**, the tank's return driving its own input, which is a signal path the box
  did not have at all and is therefore a declared departure under the owner's ruling rather than a
  fixed quantity opened. Its precedent is in the plugin's own `AGENTS.md` (since 2026-10-06 its
  [`NOTES.md` § The precedent](../../plugins/mxm-folded-spring/NOTES.md#the-precedent-researched-2026-09-04-before-it-was-built)): dub practice, the
  Doepfer A-199, Music Thing's Mk2 and Gamechanger's Light Pedal all ship it, and all of them sing
  at the top of the control.
- **The tanks are named by their length and not after boxes.** `Medium` is the measured one; `Short`
  and `Long` are chosen scalings of it, and naming them after real units would claim captures that
  do not exist. `crates/mxm-mono-00-dsp`'s `effects` module (in mxm-mono-00) says the same in its
  own words.

## 10. Minimum size and 200% scale

**Resizable**; the editor's `REFERENCE` and `MINIMUM` and their tests hold the sizes, the minimum
including the control/display floor and shell gutters. One Effects item uses the shared paging renderer without a bar. Existing painted
label, display and panel-fit tests remain. Zoom is independently chosen at **75–200%**; an
indivisible overflow scrolls. Keep physical window size fixed for §15's DPI/zoom gate rather than
doubling the window to claim fit. Native-window, real-DAW and owner inspection remain open.

## 11. The Off state

Level at zero is Off, so the editor shows a state the parameters alone imply:

- The tank's springs **go dark** — they are drawn at the rail's colour, not the accent.
- The caption's decay is the **effective** one, so raising Feedback is visible as a longer T60
  rather than only audible.
- A badge reading **Off** sits at the display's bottom-right, with the sentence *Level is at zero,
  so the tank is empty and costs no CPU* on hover.
- The Level knob keeps its normal appearance. It is not disabled — it is the way out.

Two channels, never hue alone: the badge is text, the springs' change is a fill.

## Deliberate deviations from the design system

| § | Rule | Deviation | Why |
|---|---|---|---|
| §14 | The brief is written before the editor | Written alongside it | Above: the shape was settled by the chorus's brief, and what is new here — the display and the switch — was designed in this document first |
| §5.3 | Each instrument has an identity accent | The collection accent is kept | §7 |
| §6 | Views bar | No view bar | §6 |
| §14.5 | Advanced controls and disclosure | No advanced zone | §5 |

## Sign-off checklist

- [x] §14's ten questions answered.
- [x] The one visualization is fed by the audio thread's own value, and its geometry by the DSP's.
- [x] The Off state is carried by two channels, not hue.
- [x] The size is pinned by a test rather than chosen.
- [x] What is added to the hardware layout is argued, not silent.
