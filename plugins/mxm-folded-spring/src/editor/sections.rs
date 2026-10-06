//! The parameter table and the one card, with the sentence each control's tooltip carries.
//!
//! [`all_parameters`] is also what `preset.rs` walks — capture, Init, resolve and the dirty
//! baseline all iterate it — so a parameter missing from [`ALL_IDS`] would silently fall out of
//! every preset, which is what `every_parameter_is_bound_exactly_once` guards.

use std::collections::HashMap;

use egui::{Rect, Ui};
use mxm_mono_00_dsp::effects::LONGEST_TRANSIT_MS;
use mxm_ui::control::Size;
use mxm_ui::space::{SPACE_2, SPACE_5};
use mxm_ui::theme::Tokens;
use mxm_ui::tree::{self, Height, Kind, Node};
use nice_plug::prelude::ParamSetter;

use super::binding::{Bound, segmented};
use crate::params::MxmFoldedSpringParams;
use crate::telemetry::Telemetry;

/// The knobs' size. Level is the whole performance and Feedback is performed with it, so both
/// take the biggest the system has — and one row of one size is what keeps their values on one
/// line (`mxm-chorus-06`'s editor learned that the hard way).
const LEVEL_SIZE: Size = Size::Primary;
/// A spring's line and the gap between two of them.
const SPRING_THICKNESS: f32 = 3.0;
const SPRING_GAP: f32 = 16.0;
/// The size of the display's own small text.
const CAPTION_FONT: f32 = 10.0;
/// A floor under the display, so a future control size cannot squeeze the springs together.
const MIN_TANK_BOX: f32 = 88.0;
/// How far the caption sits in from the box's bottom corners.
const CAPTION_INSET: f32 = 9.0;
/// The narrowest the display may get before a spring's length stops reading as a length. With the
/// controls beside it, it sets the card's floor; its height is the controls', at least
/// [`MIN_TANK_BOX`].
pub const TANK_MIN_WIDTH: f32 = 220.0;
/// The card's title.
pub const TITLE: &str = "Spring";

/// What a leaf of the card draws. Hashed by what it names, which keeps its widget ids stable.
#[derive(Clone, Copy, Debug, Hash)]
pub enum Leaf {
    Knob(&'static str),
    Tank,
    Display,
}

/// The one card's body, as a tree (plans/plan-layout-tree.md): described once, and that one
/// description is both measured — the card's floor and height — and drawn, leaf by leaf, through
/// the bindings ([`paint`]).
///
/// **Side by side**, as `mxm-chorus-06` has it and for the same reason: the window's width is set
/// by the collection's app bar, which is wider than two controls need, and a display stacked under
/// them would be a long thin box under a short column. Top-aligned: a knob's column is a grid built
/// from the top down, and a row that centred its children would move each half by half its own
/// height. The tank switch stands beside the knobs, so `mxm-ui` drops its label onto the knobs'
/// name line and its cells onto the circles' centre; `SPACE_5` beyond the row's own spacing, the
/// tank takes the rest of the row and the controls' height.
pub fn card(ui: &Ui, params: &MxmFoldedSpringParams) -> Node<Leaf> {
    let gap = ui.spacing().item_spacing.x;
    // A knob's column: its circle and `SPACE_5` of air.
    let column = mxm_ui::control::knob_column(LEVEL_SIZE);
    let knob = |id: &'static str| {
        let param = binding_for(id, params).param;
        tree::leaf(
            Leaf::Knob(id),
            Kind::Knob {
                name: param.name().to_owned(),
                widest: mxm_ui::control::widest_value(|n| param.format(n as f32)),
                size: LEVEL_SIZE,
                column,
            },
        )
    };
    let controls = tree::row_gap(
        gap,
        vec![
            knob("level"),
            knob("feedback"),
            tree::pad_all(
                0.0,
                SPACE_2,
                0.0,
                tree::leaf(
                    Leaf::Tank,
                    Kind::Segmented {
                        label: binding_for("tank", params).param.name().to_owned(),
                        options: TANK_LABELS.iter().map(|&label| label.to_owned()).collect(),
                        beside: Some(LEVEL_SIZE),
                    },
                ),
            ),
        ],
    );
    tree::row_gap(
        gap + SPACE_5,
        vec![
            controls,
            tree::leaf(
                Leaf::Display,
                Kind::Custom {
                    min_width: TANK_MIN_WIDTH,
                    height: Height::Row(MIN_TANK_BOX),
                    fills: true,
                },
            ),
        ],
    )
}

/// The authored card, its floor computed from its tree in `ui`'s fonts. Also what the keyboard
/// cursor is given: one card means the `Shift` tier has nowhere to go, and the parameter and value
/// tiers work exactly as they do on a paged editor.
pub fn page_items(ui: &Ui, params: &MxmFoldedSpringParams) -> Vec<mxm_ui::paging::Item<'static>> {
    use mxm_ui::{
        flow::Card,
        paging::{Category, Item, Key},
    };
    let floor = tree::card_floor(ui, TITLE, &card(ui, params));
    vec![Item {
        key: Key(0),
        // Exactly as wide as its content: the ceiling is the floor (`plans/plan-editor-standard.md`
        // A1).
        card: Card::new(TITLE, floor).capped(floor),
        category: Category::Effects,
        kind: TITLE,
    }]
}

/// Everything a leaf draws with, and the ring level read once before the frame
/// (mxm-kit's `docs/plugin-conventions.md`, *Editor contract*: destructive telemetry is read once).
pub struct Live<'a, 'b> {
    pub params: &'a MxmFoldedSpringParams,
    pub ring: f32,
    pub setter: &'a ParamSetter<'b>,
    pub text_entry: &'a mut HashMap<&'static str, Option<String>>,
}

/// Draws one leaf, in the `Ui` the tree bounded to `rect`, through the bindings — so the controls,
/// their gestures and their names are exactly what they were.
pub fn paint(ui: &mut Ui, tokens: &Tokens, leaf: &Leaf, rect: Rect, live: &mut Live<'_, '_>) {
    let params = live.params;
    match *leaf {
        Leaf::Knob(id) => binding_for(id, params).knob(
            ui,
            tokens,
            live.setter,
            LEVEL_SIZE,
            rect.width(),
            live.text_entry,
        ),
        Leaf::Tank => segmented(
            ui,
            tokens,
            "tank",
            binding_for("tank", params).param,
            &TANK_LABELS,
            Some(LEVEL_SIZE),
            &TANK_DETAILS,
            live.setter,
        ),
        Leaf::Display => tank_display(ui, tokens, params, live.ring, rect.height()),
    }
}

/// The whole view: the one card, through the shared paging renderer. Returns the bottom of the
/// card, which is what the fit test measures.
pub fn spring_card(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmFoldedSpringParams,
    telemetry: &Telemetry,
    setter: &ParamSetter<'_>,
    text_entry: &mut HashMap<&'static str, Option<String>>,
) -> f32 {
    let items = page_items(ui, params);
    let text_editing = text_entry.values().any(Option::is_some);
    let mut live = Live {
        params,
        ring: telemetry.take_ring(),
        setter,
        text_entry,
    };
    let report = mxm_ui::paging::editor::show(
        ui,
        tokens,
        &items,
        &[],
        text_editing,
        &mut |ui, _| card(ui, params),
        &mut |ui, _, leaf, rect| paint(ui, tokens, leaf, rect, &mut live),
    );
    report
        .visible
        .iter()
        .map(|(_, r)| r.bottom())
        .fold(ui.cursor().top(), f32::max)
}

/// The switch's cells, in the order the tanks are listed: shortest first.
const TANK_LABELS: [&str; 3] = ["Short", "Medium", "Long"];
/// What each tank sounds like, in [`TANK_LABELS`]' order (design system §7.3; the owner,
/// 2026-09-27: the cells of a row do not share one sentence).
const TANK_DETAILS: [&str; 3] = [
    "A shorter spring: a tighter, quicker splash.",
    "The classic spring sound, modelled on a real tank.",
    "A longer spring: a longer, looser drip.",
];

/// The tank, drawn.
///
/// Each spring is a line as long as its transit against the longest any tank asks for, so the
/// three tanks are three visibly different lengths and a two-spring tank is visibly two springs.
/// That is the whole claim: **how long the springs are and how many there are**, which is what a
/// tank is and what the switch is choosing between.
///
/// The lines brighten with [`Telemetry::take_ring`] — the wet the audio thread actually added,
/// after the level and the tank-change fade. So a tank that has snapped to silence goes dark,
/// which the level knob cannot tell you, and a tank swapped mid-tail dims through the swap.
fn tank_display(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmFoldedSpringParams,
    ring: f32,
    height: f32,
) {
    let tank = params.tank.value().model().tank();
    let engaged = params.level.value() > 0.0;
    // What the tank's own decay becomes once its return is feeding its driver: each lap costs
    // `1 - g` of what is left, so the decay stretches by `1 / (1 - g)`. The caption says the
    // number the ear will hear rather than the one on the schematic.
    let decay = crate::effective_decay_s(params.tank.value(), params.feedback.value());
    // A little compression, so the quiet end of a decay still moves the display.
    let ring = ring.clamp(0.0, 1.0).powf(0.4);

    let size = egui::vec2(ui.available_width(), height.max(MIN_TANK_BOX));
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, tokens.surface_2);

    let span = rect.width() - 2.0 * CAPTION_INSET - 24.0;
    let left = rect.left() + CAPTION_INSET + 18.0;
    let middle = rect.center().y;
    let first = middle - SPRING_GAP * (tank.springs as f32 - 1.0) / 2.0;

    for index in 0..tank.springs {
        let y = first + SPRING_GAP * index as f32;
        let length = span * (tank.transits_ms[index] / LONGEST_TRANSIT_MS);
        // The rail: how long a spring *could* be, so a short tank reads as short rather than as a
        // display that has been rescaled.
        painter.line_segment(
            [egui::pos2(left, y), egui::pos2(left + span, y)],
            egui::Stroke::new(1.0, tokens.border),
        );
        let colour = if engaged {
            lerp_colour(tokens.track, tokens.accent, ring)
        } else {
            tokens.track
        };
        painter.line_segment(
            [egui::pos2(left, y), egui::pos2(left + length, y)],
            egui::Stroke::new(SPRING_THICKNESS, colour),
        );
        // The far end, where the transducer is: a dot, so a line's end is a place and not a stop.
        painter.circle_filled(egui::pos2(left + length, y), SPRING_THICKNESS, colour);
    }

    let label_colour = if engaged {
        tokens.text_secondary
    } else {
        tokens.text_disabled
    };
    let caption_y = rect.bottom() - CAPTION_INSET;
    let shortest = tank.transits_ms[0];
    let longest = tank.transits_ms[tank.springs - 1];
    painter.text(
        egui::pos2(rect.left() + CAPTION_INSET, caption_y),
        egui::Align2::LEFT_CENTER,
        format!(
            "{} springs · {shortest:.0}–{longest:.0} ms · T60 {decay:.1} s",
            tank.springs
        ),
        egui::FontId::proportional(CAPTION_FONT),
        label_colour,
    );
    if !engaged {
        painter.text(
            egui::pos2(rect.right() - CAPTION_INSET, caption_y),
            egui::Align2::RIGHT_CENTER,
            "Off",
            egui::FontId::proportional(CAPTION_FONT),
            tokens.text_primary,
        );
    }

    response.on_hover_text(if engaged {
        "The springs in the tank; the brightness shows how much they are ringing."
    } else {
        "Level is at zero, so the reverb is off. Turn Level up to hear it."
    });
}

/// Mixes two colours, `t` of the way from `a` to `b`.
fn lerp_colour(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t) as u8;
    egui::Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// One parameter's binding, with the sentence §7.1 requires in its tooltip.
///
/// **The descriptions live here because only the plugin has them.** CLAP carries no such field, so
/// a host cannot supply one — which is the concrete reason an editor belongs to the plugin.
pub fn binding_for<'a>(id: &'static str, p: &'a MxmFoldedSpringParams) -> Bound<'a> {
    let (param, description): (&'a dyn super::binding::ErasedParam, &'static str) = match id {
        "level" => (&p.level, "How much spring reverb; at zero it is off."),
        "tank" => (&p.tank, "Which spring tank; its length sets the sound."),
        "feedback" => (
            &p.feedback,
            "Feeds the reverb back into itself, longer and darker; past about three quarters it sings.",
        ),
        other => unreachable!("no binding for parameter `{other}`"),
    };
    Bound::new(id, param, description)
}

/// Every parameter, bound, in the plugin's own order.
pub fn all_parameters(params: &MxmFoldedSpringParams) -> Vec<Bound<'_>> {
    ALL_IDS.iter().map(|id| binding_for(id, params)).collect()
}

/// Every id this plugin has. One list, so the coverage test and the lookup cannot disagree.
pub const ALL_IDS: &[&str] = &["level", "tank", "feedback"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Tank;
    use mxm_mono_00_dsp::effects::SpringTank;

    /// Every parameter the derive declares is bound exactly once, and nothing is bound that is
    /// not declared.
    #[test]
    fn every_parameter_is_bound_exactly_once() {
        use nice_plug::prelude::Params;
        let params = MxmFoldedSpringParams::default();
        let declared: Vec<String> = params
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        for id in &declared {
            let count = ALL_IDS.iter().filter(|d| *d == id).count();
            assert_eq!(count, 1, "{id} is bound {count} times, expected once");
        }
        assert_eq!(
            ALL_IDS.len(),
            declared.len(),
            "ALL_IDS {ALL_IDS:?} against declared {declared:?}"
        );
    }

    /// Every binding resolves and carries a sentence, which `binding_for`'s `unreachable!` would
    /// otherwise turn into a panic inside a paint call — and a panic there takes the host down.
    #[test]
    fn every_bound_parameter_has_a_sentence() {
        let params = MxmFoldedSpringParams::default();
        for id in ALL_IDS {
            let bound = binding_for(id, &params);
            assert!(!bound.description.is_empty(), "{id} has no description");
            assert!(
                bound.description.ends_with('.'),
                "{id}'s description is not a sentence: {:?}",
                bound.description
            );
        }
    }

    /// The caption's decay follows the feedback, because that is what a listener hears.
    #[test]
    fn the_caption_reports_the_decay_the_feedback_makes() {
        let bare = crate::effective_decay_s(Tank::Medium, 0.0);
        assert!(
            (bare - 3.5).abs() < 1e-6,
            "with no feedback it is the tank's"
        );
        assert!(
            crate::effective_decay_s(Tank::Medium, 0.5) > bare,
            "feeding the tank its own return lengthens the decay"
        );
    }

    /// The switch's cells and the parameter's tanks are the same list in the same order. A cell
    /// that named the wrong tank would be a control that lies about what it selects.
    #[test]
    fn the_switch_lists_every_tank_in_order() {
        assert_eq!(TANK_LABELS.len(), Tank::ALL.len());
        for (label, tank) in TANK_LABELS.iter().zip(Tank::ALL) {
            assert_eq!(*label, tank.model().label());
        }
    }

    /// No tank asks for a spring longer than the display's own scale, or a line would run off it.
    #[test]
    fn every_springs_transit_fits_the_displays_scale() {
        for tank in Tank::ALL {
            let tank = tank.model().tank();
            for index in 0..tank.springs {
                assert!(
                    tank.transits_ms[index] <= LONGEST_TRANSIT_MS,
                    "a spring at {} ms against a scale of {LONGEST_TRANSIT_MS}",
                    tank.transits_ms[index]
                );
            }
        }
    }

    /// The display reads `SpringTank` directly, so this is the compile-time reminder that it is
    /// the DSP's own geometry being drawn rather than numbers copied into the editor.
    #[test]
    fn the_display_draws_the_dsps_own_geometry() {
        let tank: SpringTank = Tank::Medium.model().tank();
        assert_eq!(tank.springs, 3);
        assert!(tank.t60_s > 0.0);
    }
}
