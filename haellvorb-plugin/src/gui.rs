//! Haellvorb GUI (egui) — dark, ominous reverb controls.
//! Fixed-position, scalable UI (corner scales, not resizes). Values shown only while adjusting.

use std::sync::Arc;

use nice_plug::prelude::{Param, ParamSetter};
use nice_plug_egui::{create_egui_editor, resizable_window::ResizableWindow, EguiState};

use crate::{HaellvorbParams, DEFAULT_GUI_SIZE};

const BACKGROUND_WEBP: &[u8] = include_bytes!("../assets/background.webp");

fn load_background(ctx: &egui::Context) {
    let has = ctx.data(|d| d.get_temp::<Option<egui::TextureHandle>>(egui::Id::new("haellvorb-bg")).is_some());
    if has {
        return;
    }
    let tex = (|| {
        let img = image::load_from_memory(BACKGROUND_WEBP).ok()?;
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_flat_samples().as_slice());
        Some(ctx.load_texture("haellvorb-bg", color, egui::TextureOptions::LINEAR))
    })();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("haellvorb-bg"), tex));
}

fn get_background(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    ctx.data(|d| d.get_temp::<Option<egui::TextureHandle>>(egui::Id::new("haellvorb-bg")).flatten())
}

fn set_normalized<P: Param>(param: &P, setter: &ParamSetter, norm: f32) {
    let value = param.preview_plain(norm.clamp(0.0, 1.0));
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

struct KnobStyle {
    radius: f32,
    value_font: f32,
    title_font: f32,
    end_font: f32,
    readout_gap: f32,
}

fn draw_knob_at<P, F>(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    param: &P,
    setter: &ParamSetter,
    title: &str,
    left_end: &str,
    right_end: &str,
    format_value: F,
    style: KnobStyle,
) where
    P: Param<Plain = f32>,
    F: Fn(f32) -> String,
{
    let norm = param.modulated_normalized_value();
    let plain = param.modulated_plain_value();
    let label = format_value(plain);

    let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

    if response.drag_started() || response.clicked() {
        setter.begin_set_parameter(param);
    }

    let mut new_norm = norm;
    let mut adjusted = false;
    if response.dragged() {
        let delta = response.drag_delta().y;
        if delta.abs() > 0.0 {
            new_norm = (norm - delta * 0.004).clamp(0.0, 1.0);
            set_normalized(param, setter, new_norm);
            adjusted = true;
        }
    } else if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let center_y = rect.center().y;
            let target = if pos.y < center_y { norm + 0.05 } else { norm - 0.05 };
            new_norm = target.clamp(0.0, 1.0);
            set_normalized(param, setter, new_norm);
            adjusted = true;
        }
    }
    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if response.hovered() && scroll.abs() > 0.0 {
        new_norm = (new_norm + scroll.signum() * 0.02).clamp(0.0, 1.0);
        set_normalized(param, setter, new_norm);
        adjusted = true;
    }
    if response.drag_stopped() || response.clicked() {
        setter.end_set_parameter(param);
    }

    let active_norm = if adjusted || response.dragged() {
        new_norm
    } else {
        norm
    };
    let show_value = response.dragged() || response.is_pointer_button_down_on() || (response.hovered() && scroll.abs() > 0.0);

    let p = ui.painter_at(rect);
    let accent = egui::Color32::from_rgb(160, 34, 28);
    let accent_glow = egui::Color32::from_rgb(90, 18, 16);
    let text_dim = egui::Color32::from_rgb(170, 160, 155);
    let text_mid = egui::Color32::from_rgb(190, 182, 176);
    let text_hot = egui::Color32::from_rgb(235, 226, 218);

    let knob_center = egui::pos2(rect.center().x, rect.top() + rect.height() * 0.42);
    let radius = style.radius;

    let start_angle = 150.0_f32.to_radians();
    let end_angle = 390.0_f32.to_radians();
    let sweep = end_angle - start_angle;

    // Bezel / shadow
    p.circle_filled(knob_center + egui::vec2(0.0, 3.0), radius + 8.0, egui::Color32::from_rgb(4, 4, 5));
    p.circle_filled(knob_center, radius + 7.0, egui::Color32::from_rgb(18, 19, 22));
    p.circle_stroke(
        knob_center,
        radius + 7.0,
        egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(45, 40, 40)),
    );

    // Ticks
    let ticks = 25;
    for i in 0..ticks {
        let t = i as f32 / (ticks - 1) as f32;
        let a = start_angle + sweep * t;
        let (s, c) = a.sin_cos();
        let dir = egui::vec2(c, s);
        let major = i % 4 == 0;
        let r0 = if major { radius + 1.0 } else { radius + 3.0 };
        let r1 = radius + 6.0;
        let col = if t <= active_norm {
            accent
        } else if major {
            egui::Color32::from_rgb(70, 64, 64)
        } else {
            egui::Color32::from_rgb(40, 38, 38)
        };
        let wdt = if major { 1.3_f32 } else { 1.0_f32 };
        p.line_segment([knob_center + dir * r0, knob_center + dir * r1], egui::Stroke::new(wdt, col));
    }

    // Value arc
    let arc_r = radius + 4.0;
    let steps = 48;
    let active_steps = ((active_norm * steps as f32).round() as i32).clamp(0, steps);
    if active_steps > 0 {
        let mut pts: Vec<egui::Pos2> = Vec::with_capacity(active_steps as usize + 1);
        for i in 0..=active_steps {
            let tt = (i as f32 / steps as f32).min(active_norm);
            let aa = start_angle + sweep * tt;
            let (s, c) = aa.sin_cos();
            pts.push(knob_center + egui::vec2(c, s) * arc_r);
        }
        if pts.len() >= 2 {
            p.add(egui::Shape::line(pts, egui::Stroke::new(2.0_f32, accent)));
        }
    }

    // Body
    p.circle_filled(knob_center, radius, egui::Color32::from_rgb(22, 23, 27));
    p.circle_filled(knob_center, radius - 3.0, egui::Color32::from_rgb(28, 29, 34));
    p.circle_filled(knob_center, radius - 6.0, egui::Color32::from_rgb(20, 21, 25));
    p.circle_stroke(knob_center, radius, egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(55, 50, 50)));
    p.circle_stroke(
        knob_center,
        radius - 6.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(40, 38, 40)),
    );

    // Cap
    let cap_r = radius * 0.28;
    p.circle_filled(knob_center, cap_r + 3.0, egui::Color32::from_rgb(16, 16, 18));
    p.circle_filled(knob_center, cap_r, egui::Color32::from_rgb(32, 30, 30));
    p.circle_stroke(knob_center, cap_r, egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(60, 50, 50)));
    p.circle_filled(knob_center, cap_r * 0.35, accent_glow);

    // Pointer
    let ang = start_angle + sweep * active_norm;
    let (s, c) = ang.sin_cos();
    let dir = egui::vec2(c, s);
    let p0 = knob_center + dir * (cap_r + 4.0);
    let p1 = knob_center + dir * (radius - 7.0);
    p.line_segment([p0, p1], egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(235, 225, 215)));
    p.line_segment([p0, p1], egui::Stroke::new(1.4_f32, accent));
    p.circle_filled(p1, 2.5, egui::Color32::from_rgb(235, 225, 215));

    // Labels / readout (value only while adjusting this knob)
    let readout_y = rect.bottom() - style.readout_gap;
    p.text(
        egui::pos2(rect.center().x, readout_y - style.value_font * 0.9),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(style.title_font),
        text_mid,
    );
    if show_value {
        p.text(
            egui::pos2(rect.center().x, readout_y),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::monospace(style.value_font),
            text_hot,
        );
    }

    let (ls, lc) = start_angle.sin_cos();
    let (es, ec) = end_angle.sin_cos();
    let lbl_r = radius + 12.0;
    p.text(
        knob_center + egui::vec2(lc, ls) * lbl_r + egui::vec2(-3.0, 4.0),
        egui::Align2::CENTER_CENTER,
        left_end,
        egui::FontId::proportional(style.end_font),
        text_dim,
    );
    p.text(
        knob_center + egui::vec2(ec, es) * lbl_r + egui::vec2(3.0, 4.0),
        egui::Align2::CENTER_CENTER,
        right_end,
        egui::FontId::proportional(style.end_font),
        text_dim,
    );

    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
}

pub fn create(
    params: Arc<HaellvorbParams>,
    editor_state: Arc<EguiState>,
) -> Option<Box<dyn nice_plug::prelude::Editor>> {
    let editor_state_for_ui = editor_state.clone();
    create_egui_editor(
        editor_state,
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
            load_background(ctx);
        },
        move |ui, setter, _queue, _state| {
            let base_size = egui::vec2(DEFAULT_GUI_SIZE.0 as f32, DEFAULT_GUI_SIZE.1 as f32);
            let aspect_ratio = base_size.x / base_size.y;
            ResizableWindow::new("haellvorb-main")
                .min_size(base_size * 0.6)
                .aspect_ratio(aspect_ratio)
                .show(ui, editor_state_for_ui.as_ref(), |ui| {
                    let available = ui.max_rect();
                    // Uniform scale, preserving aspect ratio
                    let scale = ((available.width() / base_size.x).min(available.height() / base_size.y)).clamp(0.6, 2.5);
                    let scaled_size = egui::vec2(base_size.x * scale, base_size.y * scale);
                    let origin = egui::pos2(
                        available.min.x + (available.width() - scaled_size.x) * 0.5,
                        available.min.y + (available.height() - scaled_size.y) * 0.5,
                    );
                    let content_rect = egui::Rect::from_min_size(origin, scaled_size);

                    let painter = ui.painter_at(available);
                    if let Some(bg) = get_background(ui.ctx()) {
                        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        painter.image(bg.id(), available, uv, egui::Color32::WHITE);
                        painter.rect_filled(available, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 90));
                    } else {
                        painter.rect_filled(available, 0.0, egui::Color32::from_rgb(7, 8, 10));
                    }
                    let s = |v: f32| v * scale;
                    let pos = |x: f32, y: f32| egui::pos2(origin.x + s(x), origin.y + s(y));
                    let vec = |w: f32, h: f32| egui::vec2(s(w), s(h));
                    let rect_at = |x: f32, y: f32, w: f32, h: f32| egui::Rect::from_min_size(pos(x, y), vec(w, h));

                    // Big DECAY knob (fixed base position, scaled with window)
                    draw_knob_at(
                        ui,
                        rect_at(150.0, 10.0, 180.0, 200.0),
                        &params.rev_decay,
                        setter,
                        "DECAY",
                        "SHORT",
                        "LONG",
                        |secs| {
                            if secs < 1.0 {
                                format!("{}", (secs * 1000.0).round() as i32)
                            } else {
                                format!("{:.1}", secs)
                            }
                        },
                        KnobStyle { radius: s(58.0), value_font: s(20.0), title_font: s(12.0), end_font: s(9.0), readout_gap: s(22.0) },
                    );

                    // Small knobs row 1 (reverb/gain) - fixed base positions
                    let y1 = 220.0;
                    let w1 = 88.0;
                    let h1 = 120.0;
                    let x = |i: f32| 10.0 + i * 92.0;
                    draw_knob_at(ui, rect_at(x(0.0), y1, w1, h1), &params.rev_mix, setter, "MIX", "DRY", "WET", |v| format!("{}", (v * 100.0).round() as i32), KnobStyle { radius: s(28.0), value_font: s(13.0), title_font: s(10.0), end_font: s(8.0), readout_gap: s(14.0) });
                    draw_knob_at(ui, rect_at(x(1.0), y1, w1, h1), &params.rev_predelay, setter, "PRE", "0", "250", |v| format!("{}", v.round() as i32), KnobStyle { radius: s(28.0), value_font: s(13.0), title_font: s(10.0), end_font: s(8.0), readout_gap: s(14.0) });
                    draw_knob_at(ui, rect_at(x(2.0), y1, w1, h1), &params.rev_damp, setter, "DAMP", "DARK", "BRIGHT", |v| format!("{}", (v * 100.0).round() as i32), KnobStyle { radius: s(28.0), value_font: s(13.0), title_font: s(10.0), end_font: s(8.0), readout_gap: s(14.0) });
                    draw_knob_at(ui, rect_at(x(3.0), y1, w1, h1), &params.in_gain, setter, "IN", "-24", "24", |v| format!("{:.1}", v), KnobStyle { radius: s(28.0), value_font: s(13.0), title_font: s(10.0), end_font: s(8.0), readout_gap: s(14.0) });
                    draw_knob_at(ui, rect_at(x(4.0), y1, w1, h1), &params.out_gain, setter, "OUT", "-24", "24", |v| format!("{:.1}", v), KnobStyle { radius: s(28.0), value_font: s(13.0), title_font: s(10.0), end_font: s(8.0), readout_gap: s(14.0) });

                    // Small knobs row 2 (scream) - fixed base positions
                    let y2 = 350.0;
                    let xs = |i: f32| 102.0 + i * 92.0;
                    draw_knob_at(ui, rect_at(xs(0.0), y2, w1, h1), &params.scream_sens, setter, "SENS", "LOW", "HIGH", |v| format!("{}", (v * 100.0).round() as i32), KnobStyle { radius: s(28.0), value_font: s(13.0), title_font: s(10.0), end_font: s(8.0), readout_gap: s(14.0) });
                    draw_knob_at(ui, rect_at(xs(1.0), y2, w1, h1), &params.scream_change, setter, "CHAOS", "CALM", "WILD", |v| format!("{}", (v * 100.0).round() as i32), KnobStyle { radius: s(28.0), value_font: s(13.0), title_font: s(10.0), end_font: s(8.0), readout_gap: s(14.0) });
                    draw_knob_at(ui, rect_at(xs(2.0), y2, w1, h1), &params.scream_gain, setter, "SCREAM", "0", "4", |v| format!("{:.2}", v), KnobStyle { radius: s(28.0), value_font: s(13.0), title_font: s(10.0), end_font: s(8.0), readout_gap: s(14.0) });
                });
        },
    )
}
