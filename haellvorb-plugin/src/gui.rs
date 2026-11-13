//! Haellvorb GUI (egui) — dark, ominous, single huge decay knob.

use std::sync::Arc;

use nice_plug::prelude::{Param, ParamSetter};
use nice_plug_egui::{create_egui_editor, resizable_window::ResizableWindow, EguiState};

use crate::{HaellvorbParams, DEFAULT_GUI_SIZE};

fn set_normalized<P: nice_plug::prelude::Param>(param: &P, setter: &ParamSetter, norm: f32) {
    let value = param.preview_plain(norm.clamp(0.0, 1.0));
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

/// Huge vertical "reverb time" control (decay) filling most of the window.
fn huge_decay_knob(ui: &mut egui::Ui, params: &HaellvorbParams, setter: &ParamSetter) {
    let norm = params.rev_decay.modulated_normalized_value();
    let label = params.rev_decay.to_string();

    let avail = ui.available_size();
    let w = avail.x.clamp(220.0, 460.0);
    let h = avail.y.clamp(320.0, 900.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click_and_drag());

    if response.drag_started() || response.clicked() {
        setter.begin_set_parameter(&params.rev_decay);
    }

    // Drag/click: vertical mapping (top = max, bottom = min) for an ominous monolith feel.
    let mut new_norm = norm;
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let pad = 40.0;
            let top = rect.top() + pad;
            let bot = rect.bottom() - pad;
            if bot > top {
                new_norm = 1.0 - ((pos.y - top) / (bot - top)).clamp(0.0, 1.0);
            }
        }
    }
    // Scroll wheel fine control
    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if response.hovered() && scroll.abs() > 0.0 {
        new_norm = (new_norm + scroll.signum() * 0.015).clamp(0.0, 1.0);
        set_normalized(&params.rev_decay, setter, new_norm);
    } else if response.dragged() || response.clicked() {
        if (new_norm - norm).abs() > f32::EPSILON {
            set_normalized(&params.rev_decay, setter, new_norm);
        }
    }

    if response.drag_stopped() || response.clicked() {
        setter.end_set_parameter(&params.rev_decay);
    }

    let p = ui.painter_at(rect);
    let bg = egui::Color32::from_rgb(7, 8, 10);
    let frame = egui::Color32::from_rgb(18, 20, 24);
    let stroke_col = egui::Color32::from_rgb(40, 18, 18);
    let blood = egui::Color32::from_rgb(140, 18, 18);
    let glow = egui::Color32::from_rgb(90, 10, 10);
    let text_dim = egui::Color32::from_rgb(120, 110, 110);
    let text_hot = egui::Color32::from_rgb(210, 190, 170);

    p.rect_filled(rect, 12.0, bg);
    let inner = rect.shrink(10.0);
    p.rect_filled(inner, 10.0, frame);
    p.rect_stroke(inner, 10.0, egui::Stroke::new(1.5, stroke_col), egui::StrokeKind::Inside);

    // Ominous vertical "well" / fill showing decay amount
    let well = egui::Rect::from_min_max(
        egui::pos2(inner.center().x - 46.0, inner.top() + 60.0),
        egui::pos2(inner.center().x + 46.0, inner.bottom() - 90.0),
    );
    p.rect_filled(well, 18.0, egui::Color32::from_rgb(10, 10, 12));
    p.rect_stroke(well, 18.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(35, 30, 30)), egui::StrokeKind::Inside);

    let fill_h = well.height() * new_norm.max(norm);
    let fill = egui::Rect::from_min_max(egui::pos2(well.left(), well.bottom() - fill_h), well.max);
    if fill.height() > 1.0 {
        p.rect_filled(fill, 18.0, glow);
        let core = fill.shrink2(egui::vec2(18.0, 0.0));
        if core.width() > 2.0 && core.height() > 2.0 {
            p.rect_filled(core, 12.0, blood);
        }
    }

    // Indicator notch
    let y = well.bottom() - well.height() * norm;
    p.line_segment(
        [egui::pos2(well.left() - 10.0, y), egui::pos2(well.right() + 10.0, y)],
        egui::Stroke::new(2.0, text_hot),
    );

    // Title and readout
    let title = "DECAY";
    p.text(
        egui::pos2(inner.center().x, inner.top() + 28.0),
        egui::Align2::CENTER_CENTER,
        "HAELLVÖRB",
        egui::FontId::proportional(22.0),
        text_dim,
    );
    p.text(
        egui::pos2(inner.center().x, inner.bottom() - 58.0),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(18.0),
        text_dim,
    );
    p.text(
        egui::pos2(inner.center().x, inner.bottom() - 30.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::monospace(20.0),
        text_hot,
    );

    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
}

pub fn create(
    params: Arc<HaellvorbParams>,
    editor_state: Arc<EguiState>,
) -> Option<Box<dyn nice_plug::prelude::Editor>> {
    create_egui_editor(
        editor_state.clone(),
        (),
        Default::default(),
        |ctx, _queue, _state| {
            let mut visuals = egui::Visuals::dark();
            visuals.override_text_color = Some(egui::Color32::from_rgb(170, 160, 155));
            visuals.panel_fill = egui::Color32::from_rgb(7, 8, 10);
            visuals.window_fill = egui::Color32::from_rgb(7, 8, 10);
            visuals.extreme_bg_color = egui::Color32::from_rgb(10, 10, 12);
            visuals.faint_bg_color = egui::Color32::from_rgb(14, 14, 16);
            ctx.set_visuals(visuals);
        },
        move |ui, setter, _queue, _state| {
            ResizableWindow::new("haellvorb-main")
                .min_size(egui::vec2(DEFAULT_GUI_SIZE.0 as f32, DEFAULT_GUI_SIZE.1 as f32))
                .show(ui, editor_state.as_ref(), |ui| {
                    let panel = egui::Frame::NONE
                        .fill(egui::Color32::from_rgb(7, 8, 10))
                        .inner_margin(egui::Margin::symmetric(24, 18));
                    panel.show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("something is listening")
                                    .italics()
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(80, 60, 60)),
                            );
                            ui.add_space(8.0);
                            huge_decay_knob(ui, &params, setter);
                        });
                    });
                });
        },
    )
}
