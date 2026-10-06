# mxm-folded-spring

The Roland System-100 103 mixer's spring reverb as a standalone CLAP effect — **with the tank as a
control**.

Not affiliated with, endorsed by, or associated with Roland. No Roland trademark is used to name
this product.

## What it is

A spring reverb is a box of springs with a transducer at each end. The 103's is three of them at
different lengths, so a signal comes back three times at three delays, each smeared by the springs'
own dispersion — the high frequencies arrive late, which is the "boing". The model is measured from
an RE-201 capture, the family the 103's tank is inferred to belong to: transits of 41.6 and 60.0 ms
resolved, a third placed between them, a 3.5 s decay, and the tank's 450 Hz – 1.3 kHz band-pass.

**Three controls.** *Level* is the 103's one knob. *Tank* is the one the hardware could only offer
with a screwdriver:

| Tank | What it is |
|---|---|
| **Short** | Two springs, about 25 and 36 ms, ringing 1.75 s, brighter. **Chosen.** |
| **Medium** | The measured tank: three springs, 41.6 / 50.4 / 60.0 ms, 3.5 s. **The instrument's, and the default.** |
| **Long** | Three springs, 62 to 90 ms, ringing 5.95 s, darker. **Chosen.** |

`Short` and `Long` are scalings of the measured tank, not separate measurements, and they are named
after their length rather than after real units — naming them after boxes would claim captures that
do not exist.

*Feedback* is the third, and it is the one thing here the box did not have: the tank's return sent
back into its own driver. Most of its travel lengthens and darkens the decay; the **last tenth**
self-oscillates, the way a filter's resonance does, which is what every hardware version of this
control offers. That point is the same on all three tanks, which took measuring: a long tank is
four times readier to sing than a short one. It starts at zero, so the plugin as it loads is still
exactly the tank.

The DSP is `mxm-mono-00`'s own `effects` module, depended on in place rather than copied, so there
is one implementation of this reverb and the instrument's render is provably unchanged: a digest
taken on the revision before the tanks existed still passes. That test is mxm-mono-00's, and since
2026-10-06 the digest is pinned on Windows only; elsewhere that one check is skipped.

## Two layouts

**Mono in, mono out** is the tank's own topology and the default: one tank, one return.

**Stereo in, stereo out** is offered for hosts that insert on stereo tracks. The two channels are
summed into the one tank and its return is added to both, with each side's dry passing through
untouched — so the reverb is mono, as the tank is, and the source keeps its image. Off passes both
channels through to the bit.

## Level at zero is Off, and doing nothing costs nothing

There is no On switch. At zero the tank is **emptied** rather than frozen — a frozen tank replays
ten-second-old audio when the level comes back — and from the next block the reverb is not run at
all, the output being the input.

## Changing tank

A tank is delay lines read at its own transits, so it cannot be crossfaded into another. The plugin
fades the wet out over ten milliseconds, swaps the tank at silence, and fades back in. The dry is
untouched throughout, so what you hear is one tank stopping and another starting — which is what
changing a tank is.

## Presets

Four: one per tank, each at a level chosen for it, and *Regenerating* — the long tank driving
itself, which is the dub send routed back to its own return. **Init is not a file**: it is generated from the
parameter defaults. Your own presets are saved as readable JSON under the platform config directory.

## Status

**Built and tested**: the three parameters, the three tanks, four presets, state, both layouts, and
the editor — one card with the Level knob, the tank switch, and a display of the springs at their real
lengths, brightening with what the tank is actually ringing at. `clap-validator` reports 33 passed
and nothing failed. Not yet run in a commercial host.

**Fidelity is UNVERIFIED.** No hardware was measured here; the tank is modelled from a capture of a
different unit in the same family, and `crates/mxm-mono-00-dsp`'s `AGENTS.md` lists every constant
that is chosen rather than measured (in mxm-mono-00; the list is now its
[`NOTES.md` § What is chosen, not measured](https://github.com/mxm-audio/mxm-mono-00/blob/main/crates/mxm-mono-00-dsp/NOTES.md#what-is-chosen-not-measured)). Two of the three tanks are chosen outright.

## Building

```bash
cargo xtask bundle mxm-folded-spring --release
```
