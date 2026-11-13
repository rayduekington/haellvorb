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
    let secs = params.rev_decay.value();
    let label = if secs < 1.0 {
        format!("{} MS", (secs * 1000.0).round() as i32)
    } else {
        format!("{} S", secs.round() as i32)
    };

    let avail = ui.available_size();
    let w = avail.x.clamp(240.0, 420.0);
    let h = avail.y.clamp(340.0, 900.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click_and_drag());

    if response.drag_started() || response.clicked() {
        setter.begin_set_parameter(&params.rev_decay);
    }

    // Drag/click: vertical mapping (top = max, bottom = min).
    let mut new_norm = norm;
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let pad = 46.0;
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

    let active_norm = if response.dragged() || response.clicked() { new_norm } else { norm };

    let p = ui.painter_at(rect);
    let bg = egui::Color32::from_rgb(6, 7, 9);
    let frame = egui::Color32::from_rgb(14, 15, 18);
    let stroke_col = egui::Color32::from_rgb(38, 34, 34);
    let accent = egui::Color32::from_rgb(150, 28, 28);
    let accent_dim = egui::Color32::from_rgb(80, 18, 18);
    let text_dim = egui::Color32::from_rgb(110, 104, 104);
    let text_mid = egui::Color32::from_rgb(150, 142, 138);
    let text_hot = egui::Color32::from_rgb(225, 214, 206);

    p.rect_filled(rect, 8.0, bg);
    let inner = rect.shrink(12.0);
    p.rect_filled(inner, 6.0, frame);
    p.rect_stroke(inner, 6.0, egui::Stroke::new(1.0, stroke_col), egui::StrokeKind::Inside);

    // Vertical well
    let well = egui::Rect::from_min_max(
        egui::pos2(inner.center().x - 38.0, inner.top() + 72.0),
        egui::pos2(inner.center().x + 38.0, inner.bottom() - 96.0),
    );
    p.rect_filled(well, 10.0, egui::Color32::from_rgb(9, 10, 12));
    p.rect_stroke(well, 10.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(30, 28, 28)), egui::StrokeKind::Inside);

    // Subtle scale ticks
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let yy = well.bottom() - well.height() * t;
        let major = i % 5 == 0;
        let len = if major { 10.0 } else { 5.0 };
        let col = if major { egui::Color32::from_rgb(55, 50, 50) } else { egui::Color32::from_rgb(35, 33, 33) };
        p.line_segment(
            [egui::pos2(well.right() + 8.0, yy), egui::pos2(well.right() + 8.0 + len, yy)],
            egui::Stroke::new(1.0, col),
        );
        p.line_segment(
            [egui::pos2(well.left() - 8.0 - len, yy), egui::pos2(well.left() - 8.0, yy)],
            egui::Stroke::new(1.0, col),
        );
    }

    // Fill
    let fill_h = well.height() * active_norm;
    if fill_h > 1.0 {
        let fill = egui::Rect::from_min_max(egui::pos2(well.left(), well.bottom() - fill_h), well.max);
        p.rect_filled(fill, 10.0, accent_dim);
        let core = egui::Rect::from_min_max(
            egui::pos2(well.center().x - 10.0, fill.top()),
            egui::pos2(well.center().x + 10.0, fill.bottom()),
        );
        let core = core.intersect(well);
        if core.is_positive() {
            p.rect_filled(core, 6.0, accent);
        }
    }

    // Indicator
    let y = well.bottom() - well.height() * active_norm;
    p.line_segment(
        [egui::pos2(well.left() - 14.0, y), egui::pos2(well.right() + 14.0, y)],
        egui::Stroke::new(2.0, text_hot),
    );

    // Typography
    p.text(
        egui::pos2(inner.center().x, inner.top() + 30.0),
        egui::Align2::CENTER_CENTER,
        "HAELLVÖRB",
        egui::FontId::proportional(16.0),
        text_dim,
    );
    p.text(
        egui::pos2(inner.center().x, inner.bottom() - 62.0),
        egui::Align2::CENTER_CENTER,
        "DECAY",
        egui::FontId::proportional(13.0),
        text_mid,
    );
    p.text(
        egui::pos2(inner.center().x, inner.bottom() - 32.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::monospace(26.0),
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
                            ui.add_space(4.0);
                            huge_decay_knob(ui, &params, setter);
                            ui.add_space(4.0);
                        });
                    });
                });
        },
    )
}
