//! mxm-folded-spring's editor.
//!
//! Built to `docs/briefs/mxm-folded-spring.md`, which is the gating document — this module
//! implements it and does not re-decide it. It follows the shape `mxm-chorus-06` set for an
//! effect's editor: **one view, no view bar**, the collection accent rather than an identity hue,
//! the window's width set by the app bar, and the display beside the controls rather than under
//! them. Where this one differs is the display, which is the tank itself.
//!
//! # It is a panel, not a window
//!
//! [`panel`] takes a `Ui` and draws into it. It does not create a window, run an event loop, or own
//! a swapchain.
//!
//! # Gestures
//!
//! Every edit is bracketed: `begin_set_parameter`, `set_parameter_normalized`, `end_set_parameter`,
//! in exactly one place — [`binding::Bound::apply`]. An unclosed gesture leaves a host's automation
//! lane latched, and it breaks the player's step editing outright.
//!
//! # No developer channel
//!
//! It arrives as MIDI CC and an effect has no note port. See `telemetry.rs`.

pub mod binding;
pub mod sections;

use std::collections::HashMap;
use std::sync::Arc;

use egui::Ui;
use mxm_ui::space::SPACE_5;
use mxm_ui::theme::Tokens;
use nice_plug::context::gui::GuiContext;
use nice_plug::prelude::*;
use nice_plug_egui::{EguiEditorState, NiceEguiApp, create_egui_editor};

use crate::params::MxmFoldedSpringParams;
use crate::telemetry::Telemetry;

/// The size the editor **opens** at — **derived, not chosen**: the quarter-4K budget hugged around
/// its card (`plans/plan-editor-standard.md` F1), which `tests::the_opening_size_is_the_budget_hugged`
/// holds; `tests::the_panel_fits_the_editor` pins the height. The app bar compacts to that width
/// (design system §3.1).
const REFERENCE: (u32, u32) = (685, 250);

/// The narrowest the window may be. Below this the card's controls stop being usable, which no
/// arrangement fixes — there is only one card here, so there is nothing to rearrange (§4.3).
const MINIMUM: (u32, u32) = (683, 220);

/// Builds the editor. Called from `Plugin::editor`.
pub fn create(
    params: Arc<MxmFoldedSpringParams>,
    telemetry: Arc<Telemetry>,
) -> Option<MxmFoldedSpringEditor> {
    let state = EguiEditorState::from_size(
        nice_plug::editor::dpi::LogicalSize::new(REFERENCE.0, REFERENCE.1),
        1.0,
    );

    create_egui_editor(
        state,
        nice_plug_egui::RepaintNotifier::new(),
        nice_plug_egui::EguiNiceSettings {
            title: "mxm-folded-spring".to_owned(),
            // **Resizable**, as every editor in the collection is (`plugins/AGENTS.md`).
            //
            // There is no flow here and there does not need to be: this effect is **one card**, and
            // a single card has no row to wrap into. What resizing buys it is a window a tiling
            // manager can size, with the card taking whatever width it is given; the floor is the
            // width below which the card's own controls stop being usable (§4.3).
            resize_hint: ResizeHint {
                size_constraints: nice_plug::editor::SizeConstraints::min_logical_size(
                    nice_plug::editor::dpi::LogicalSize::new(MINIMUM.0 as f32, MINIMUM.1 as f32),
                ),
                ..ResizeHint::RESIZABLE
            },
            ..Default::default()
        },
        MxmFoldedSpringApp::new(params, telemetry),
    )
}

/// The editor type the plugin exposes.
pub type MxmFoldedSpringEditor = nice_plug_egui::EguiEditor<MxmFoldedSpringApp>;

/// Where the panel records the bottom of its content, for the fit test to read.
///
/// **Not `globally_used_rect`**: the central panel fills the window whatever is in it, so that
/// measure only exceeds the window once something is already cut off.
pub(crate) fn content_bottom_id() -> egui::Id {
    egui::Id::new("mxm-folded-spring-content-bottom")
}

/// The editor's own state: what the plugin does not own and the host does not need.
pub struct MxmFoldedSpringApp {
    params: Arc<MxmFoldedSpringParams>,
    telemetry: Arc<Telemetry>,
    /// Set in `build`, because that is where nice-plug hands it over.
    gui_context: Option<GuiContext>,
    /// Open text-entry buffers, keyed by parameter id.
    text_entry: HashMap<&'static str, Option<String>>,
    /// The preset library and everything the browser needs across frames.
    presets: PresetUi,
    /// Where the keyboard is: a card, and a parameter inside it. Transient, like the text
    /// buffers — it is not a parameter and nothing durable reads it.
    nav: mxm_ui::navigation::State,
}

pub use mxm_preset::PresetUi;

impl MxmFoldedSpringApp {
    pub fn new(params: Arc<MxmFoldedSpringParams>, telemetry: Arc<Telemetry>) -> Self {
        let params_for_presets = Arc::clone(&params);
        Self {
            params,
            telemetry,
            gui_context: None,
            text_entry: HashMap::new(),
            presets: PresetUi::new(params_for_presets.as_ref()),
            nav: mxm_ui::navigation::State::default(),
        }
    }
}

impl NiceEguiApp for MxmFoldedSpringApp {
    fn build(
        &mut self,
        egui_ctx: egui::Context,
        nice_gui_ctx: GuiContext,
        _frame: &mut nice_plug_egui::Frame,
    ) -> Result<(), nice_plug_egui::baseview::HandlerError> {
        mxm_ui::theme::apply(&egui_ctx);
        mxm_ui::typography::apply(&egui_ctx);
        // Light by default, overridable with `MXM_EDITOR_THEME`. The reasoning, and why the
        // default is not `System`, lives on `mxm_ui::theme::preference`.
        egui_ctx.set_theme(mxm_ui::theme::preference());
        self.gui_context = Some(nice_gui_ctx);
        Ok(())
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut nice_plug_egui::Frame) {
        let Some(gui_context) = self.gui_context.clone() else {
            return;
        };
        panel(
            ui,
            &self.params,
            &self.telemetry,
            &gui_context.param_setter(),
            &mut self.text_entry,
            &mut self.presets,
            &mut self.nav,
        );
    }

    fn editor_closed(&mut self) {
        self.gui_context = None;
    }
}

/// The whole editor, as a panel.
pub fn panel(
    ui: &mut Ui,
    params: &MxmFoldedSpringParams,
    telemetry: &Telemetry,
    setter: &ParamSetter<'_>,
    text_entry: &mut HashMap<&'static str, Option<String>>,
    presets: &mut PresetUi,
    nav: &mut mxm_ui::navigation::State,
) {
    let tokens = &tokens_for(ui);

    // **The springs brighten and dim without input, so the frames have to come without input too.**
    // egui repaints when something happens; a tail decaying is not something egui can see.
    ui.ctx().request_repaint();

    let peak = telemetry.take_peak();
    let clipped = telemetry.clipped();
    // One question, and both layers suspend on it: the paging renderer's `hold` and the cursor's
    // `inert` both ask whether another surface owns this frame's keyboard.
    let busy = presets.holds_the_keyboard() || text_entry.values().any(Option::is_some);
    mxm_ui::paging::editor::hold(ui.ctx(), busy);
    // **The cursor moves before anything is drawn**, so a navigation arrow is consumed here rather
    // than also walking egui's own focus ring. It reads the registry and the exact card rectangles
    // the previous frame built, and navigates the paging plan's own order.
    mxm_ui::navigation::paged(ui.ctx(), nav, busy);

    mxm_ui::AppBar::new("mxm-folded-spring").show_with(
        ui,
        tokens,
        |ui| mxm_preset::ui::preset_row(ui, tokens, params, setter, presets),
        |ui| {
            if mxm_ui::shell::level_meter(ui, tokens, peak, clipped) {
                telemetry.clear_clip();
            }
            mxm_ui::shell::zoom_control(ui);

            // §3.1 slot 5, and the same place the player keeps it: at the left end of the bar's
            // right-hand group. What the person picks is remembered for every MXM editor, so the
            // next one to open agrees with this one.
            mxm_ui::shell::editor_theme_control(ui);
        },
    );

    mxm_preset::ui::overlays(ui, tokens, params, setter, presets);

    // **No view bar** — two parameters do not divide into a designed panel and a generic list.
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(tokens.canvas)
                .inner_margin(egui::Margin::same(SPACE_5 as i8)),
        )
        .show(ui, |ui| {
            let bottom = sections::spring_card(ui, tokens, params, telemetry, setter, text_entry);
            ui.data_mut(|d| d.insert_temp(content_bottom_id(), bottom + SPACE_5));
        });
}

/// The collection's tokens, unchanged. **No identity accent**: §5.3 gives each *instrument* a hue
/// so a rack of them is tellable apart; an effect is told apart by its name in a chain.
fn tokens_for(ui: &Ui) -> Tokens {
    if ui.visuals().dark_mode {
        mxm_ui::DARK
    } else {
        mxm_ui::LIGHT
    }
}

/// The paging items as the editor computes them, from a context set up as an editor's is — three
/// passes in, so the weighted font cuts are bound — for tests, which have no editor `Ui` to hand.
#[cfg(test)]
pub(crate) fn test_items(params: &MxmFoldedSpringParams) -> Vec<mxm_ui::paging::Item<'static>> {
    let ctx = egui::Context::default();
    mxm_ui::typography::apply(&ctx);
    mxm_ui::theme::apply(&ctx);
    let mut items = Vec::new();
    for _ in 0..3 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            items = sections::page_items(ui, params);
        });
        output.textures_delta.clear();
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::params::internals::ParamPtr;
    use nice_plug::prelude::{PluginApi, PluginState};

    /// The editor reports edits through a `ParamSetter`; laying it out makes none.
    struct NoHost;

    impl nice_plug::context::gui::GuiContextInner for NoHost {
        // A test double has no host to ask for a restart (nice-plug 0.4).
        fn request_restart(&self) {}
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, _param: ParamPtr, _normalized: f32) {}
        unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}
        fn get_state(&self) -> PluginState {
            PluginState {
                version: String::new(),
                params: Default::default(),
                fields: Default::default(),
            }
        }
        fn set_state(&self, _state: PluginState) {}
    }

    /// Lays the panel out headlessly and returns the context and the bottom of its content.
    fn lay_out(width: f32, height: f32, level: f32) -> (egui::Context, f32) {
        let ctx = egui::Context::default();
        mxm_ui::theme::apply(&ctx);
        mxm_ui::typography::apply(&ctx);
        ctx.set_theme(egui::ThemePreference::Light);
        ctx.all_styles_mut(|style| style.animation_time = 0.0);

        let params = MxmFoldedSpringParams::default();
        {
            use nice_plug::params::{InternalParamMut, Param};
            let normalised = params.level.preview_normalized(level);
            unsafe {
                let _ = params.level._internal_set_normalized_value(normalised);
                params.level._internal_update_smoother(48_000.0, true);
            }
        }
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut text_entry = HashMap::new();
        // A library rooted nowhere: a layout test must never touch the real config directory.
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();

        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, height),
            )),
            ..Default::default()
        };

        for _ in 0..3 {
            let mut output = ctx.run_ui(input.clone(), |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            });
            output.textures_delta.clear();
        }
        let bottom = ctx
            .memory(|m| m.data.get_temp::<f32>(content_bottom_id()))
            .expect("the panel records where its content ends");
        (ctx, bottom)
    }

    use mxm_plugin_test::keyboard_checks;

    /// What this editor keeps behind a disclosure, opened so the reachability check sees it.
    /// Nothing here: every control is on a card.
    const REVEAL: fn(&egui::Context) = |_| {};

    /// The rollout's own failure mode: a control whose `navigation::at` scope was forgotten paints
    /// exactly as before and is simply unreachable from the keyboard. Nothing else would say so.
    #[test]
    fn the_keyboard_cursor_reaches_and_operates_every_parameter() {
        let params = MxmFoldedSpringParams::default();
        let telemetry = Telemetry::default();
        let host = keyboard_checks::Recorder::default();
        let setter = ParamSetter::new(&host);
        let ids: Vec<&str> = sections::all_parameters(&params)
            .iter()
            .map(|bound| bound.id)
            .collect();
        let mut text_entry = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        keyboard_checks::the_cursor_reaches_and_operates(
            egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
            &test_items(&params),
            keyboard_checks::Coverage::Exactly(&ids),
            &REVEAL,
            &host,
            &mut |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            },
        );
    }

    /// **The scar both mono editors carry, turned into a number.** A height guessed before the
    /// panel existed, and controls cut off the bottom of a window that cannot be resized.
    #[test]
    fn the_panel_fits_the_editor() {
        let (_, used) = lay_out(REFERENCE.0 as f32, REFERENCE.1 as f32, 0.35);
        eprintln!("the panel needs {used} points of height");
        assert!(
            used <= REFERENCE.1 as f32,
            "the panel needs {used} points of height in a {} point window; it will be clipped",
            REFERENCE.1
        );
        assert!(
            used > REFERENCE.1 as f32 - 60.0,
            "the panel needs only {used} points in a {} point window; the window is taller than it has to be",
            REFERENCE.1
        );
    }

    /// The Off state is a *layout* no-op: the display changes what it paints, never how much room
    /// it takes, or the window would resize itself when the level reached zero.
    #[test]
    fn switching_off_does_not_move_anything() {
        let (_, engaged) = lay_out(REFERENCE.0 as f32, REFERENCE.1 as f32, 0.35);
        let (_, off) = lay_out(REFERENCE.0 as f32, REFERENCE.1 as f32, 0.0);
        assert!(
            (engaged - off).abs() < 0.5,
            "the panel is {engaged} points engaged and {off} off; the Off state must not reflow"
        );
    }

    /// The card holds the controls **and** a readable tank beside them, and brief §10's minimum
    /// window holds the card: its floor — computed from its tree, the controls, the gap and the
    /// tank's [`sections::TANK_MIN_WIDTH`] — plus the panel's gutters is no wider than [`MINIMUM`].
    #[test]
    fn the_minimum_window_holds_the_card_at_its_floor() {
        let floor = test_items(&MxmFoldedSpringParams::default())[0].card.floor;
        assert!(
            floor + 2.0 * SPACE_5 <= MINIMUM.0 as f32,
            "the card's floor is {floor} points and the minimum window leaves it {}",
            MINIMUM.0 as f32 - 2.0 * SPACE_5
        );
    }

    /// Straight into a parameter, as `lay_out` does: a check has no host.
    fn set<P: nice_plug::params::InternalParamMut>(param: &P, plain: P::Plain) {
        unsafe {
            let _ = param._internal_set_plain_value(plain);
            param._internal_update_smoother(48_000.0, true);
        }
    }

    /// The card, in every state that changes what it paints, passes the layout tree's checks
    /// (plans/plan-layout-tree.md §4.3, `tree_checks::card`): its computed floor holds its content
    /// with nothing painted outside the card, the content floor is exact, the height its tree states
    /// is the height it draws, and every leaf stays in the room it was given.
    ///
    /// The states are this editor's structural-state matrix. The card has no route, disclosure or
    /// reserved alternative, so only what the tank display paints changes: the init patch; Off
    /// (Level at zero), which paints the badge; every tank, from two springs to three and from the
    /// shortest to the longest; and the tank ringing at full level with the feedback at its top,
    /// where the caption's decay is longest.
    #[test]
    fn every_card_passes_the_tree_checks_in_every_state() {
        use crate::params::Tank;
        // A state, and what it sets: the level, the tank, the feedback and the ring the display is
        // handed. `None` leaves the init patch's value.
        type State = (String, Option<f32>, Option<Tank>, Option<f32>, f32);
        let mut states: Vec<State> = vec![
            ("init".to_owned(), None, None, None, 0.0),
            ("off".to_owned(), Some(0.0), None, None, 0.0),
            (
                "ringing at the longest decay".to_owned(),
                Some(1.0),
                Some(Tank::Long),
                Some(1.0),
                1.0,
            ),
        ];
        for tank in Tank::ALL {
            let name = format!("the {} tank", tank.model().label());
            states.push((name, None, Some(tank), None, 0.5));
        }
        for (state, level, tank, feedback, ring) in states {
            let params = MxmFoldedSpringParams::default();
            if let Some(level) = level {
                set(&params.level, level);
            }
            if let Some(tank) = tank {
                set(&params.tank, tank);
            }
            if let Some(feedback) = feedback {
                set(&params.feedback, feedback);
            }
            let floor = test_items(&params)[0].card.floor;
            let host = NoHost;
            let setter = ParamSetter::new(&host);
            let mut text_entry = HashMap::new();
            let mut live = sections::Live {
                params: &params,
                ring,
                setter: &setter,
                text_entry: &mut text_entry,
            };
            tree_checks::card(
                &|_| {},
                &state,
                sections::TITLE,
                floor,
                &|ui| sections::card(ui, &params),
                &mut |ui, leaf, rect| sections::paint(ui, &mxm_ui::LIGHT, leaf, rect, &mut live),
            );
        }
    }

    use mxm_plugin_test::opening_size;
    use mxm_plugin_test::tree_checks;

    /// **The editor opens at the quarter-4K budget, hugged** (`plans/plan-editor-standard.md` F1):
    /// `REFERENCE` is derived, not typed, and this holds it.
    #[test]
    fn the_opening_size_is_the_budget_hugged() {
        let params = MxmFoldedSpringParams::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut text_entry = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        opening_size::is_the_budget_hugged(
            egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
            &REVEAL,
            &mut |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            },
        );
    }

    /// **The app bar holds in the narrowest window**: its `…` menu whole and nothing drawn over
    /// anything else, from `MINIMUM` up (`opening_size::bar_holds_from_the_minimum`).
    #[test]
    fn the_app_bar_holds_in_the_minimum_window() {
        let params = MxmFoldedSpringParams::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut text_entry = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        opening_size::bar_holds_from_the_minimum(
            egui::vec2(MINIMUM.0 as f32, MINIMUM.1 as f32),
            &mut |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            },
        );
    }

    /// The panel at the opening size, light and dark, for the owner's review of the layout-tree
    /// conversion (plans/plan-layout-tree.md §4.3): `target/layout-tree/mxm-folded-spring/<tag>/`,
    /// where `MXM_PICTURES` names the tag — `before` on the unconverted editor, `after` on the tree.
    ///
    /// `MXM_PICTURES=after cargo test -p mxm-folded-spring --lib tree_pictures -- --ignored`
    #[test]
    #[ignore = "renders through wgpu; run by hand"]
    fn tree_pictures() {
        let tag = std::env::var("MXM_PICTURES").unwrap_or_else(|_| "after".to_owned());
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/layout-tree/mxm-folded-spring")
            .join(tag);
        let params = MxmFoldedSpringParams::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut text_entry = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        tree_checks::pictures(
            &|_| {},
            egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
            &dir,
            &mut |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            },
        );
    }
}
