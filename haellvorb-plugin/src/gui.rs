//! Haellvorb GUI (egui) — dark, ominous reverb controls.
//! Big knob: reverb time (decay). Smaller knobs: other dials, same style.

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

fn draw_knob<P, F>(
    ui: &mut egui::Ui,
    param: &P,
    setter: &ParamSetter,
    title: &str,
    left_end: &str,
    right_end: &str,
    format_value: F,
    size: egui::Vec2,
    style: KnobStyle,
) where
    P: Param<Plain = f32>,
    F: Fn(f32) -> String,
{
    let norm = param.modulated_normalized_value();
    let plain = param.modulated_plain_value();
    let label = format_value(plain);

    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());

    if response.drag_started() || response.clicked() {
        setter.begin_set_parameter(param);
    }

    let mut new_norm = norm;
    if response.dragged() {
        let delta = response.drag_delta().y;
        if delta.abs() > 0.0 {
            new_norm = (norm - delta * 0.004).clamp(0.0, 1.0);
            set_normalized(param, setter, new_norm);
        }
    } else if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let center_y = rect.center().y;
            let target = if pos.y < center_y { norm + 0.05 } else { norm - 0.05 };
            new_norm = target.clamp(0.0, 1.0);
            set_normalized(param, setter, new_norm);
        }
    }
    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if response.hovered() && scroll.abs() > 0.0 {
        new_norm = (new_norm + scroll.signum() * 0.02).clamp(0.0, 1.0);
        set_normalized(param, setter, new_norm);
    }
    if response.drag_stopped() || response.clicked() {
        setter.end_set_parameter(param);
    }

    let active_norm = if response.dragged() || (response.clicked() && (new_norm - norm).abs() > f32::EPSILON) {
        new_norm
    } else {
        norm
    };

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
    p.circle_filled(knob_center + egui::vec2(0.0, 4.0), radius + 12.0, egui::Color32::from_rgb(4, 4, 5));
    p.circle_filled(knob_center, radius + 10.0, egui::Color32::from_rgb(18, 19, 22));
    p.circle_stroke(
        knob_center,
        radius + 10.0,
        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(45, 40, 40)),
    );

    // Ticks
    let ticks = 31;
    for i in 0..ticks {
        let t = i as f32 / (ticks - 1) as f32;
        let a = start_angle + sweep * t;
        let (s, c) = a.sin_cos();
        let dir = egui::vec2(c, s);
        let major = i % 5 == 0;
        let r0 = if major { radius + 1.0 } else { radius + 4.0 };
        let r1 = radius + 8.0;
        let col = if t <= active_norm {
            accent
        } else if major {
            egui::Color32::from_rgb(70, 64, 64)
        } else {
            egui::Color32::from_rgb(40, 38, 38)
        };
        let wdt = if major { 1.5_f32 } else { 1.0_f32 };
        p.line_segment([knob_center + dir * r0, knob_center + dir * r1], egui::Stroke::new(wdt, col));
    }

    // Value arc
    let arc_r = radius + 5.0;
    let steps = 60;
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
            p.add(egui::Shape::line(pts, egui::Stroke::new(2.5_f32, accent)));
        }
    }

    // Body
    p.circle_filled(knob_center, radius, egui::Color32::from_rgb(22, 23, 27));
    p.circle_filled(knob_center, radius - 3.0, egui::Color32::from_rgb(28, 29, 34));
    p.circle_filled(knob_center, radius - 7.0, egui::Color32::from_rgb(20, 21, 25));
    p.circle_stroke(knob_center, radius, egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(55, 50, 50)));
    p.circle_stroke(
        knob_center,
        radius - 7.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(40, 38, 40)),
    );

    // Cap
    let cap_r = radius * 0.28;
    p.circle_filled(knob_center, cap_r + 4.0, egui::Color32::from_rgb(16, 16, 18));
    p.circle_filled(knob_center, cap_r, egui::Color32::from_rgb(32, 30, 30));
    p.circle_stroke(knob_center, cap_r, egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(60, 50, 50)));
    p.circle_filled(knob_center, cap_r * 0.35, accent_glow);

    // Pointer
    let ang = start_angle + sweep * active_norm;
    let (s, c) = ang.sin_cos();
    let dir = egui::vec2(c, s);
    let p0 = knob_center + dir * (cap_r + 6.0);
    let p1 = knob_center + dir * (radius - 10.0);
    p.line_segment([p0, p1], egui::Stroke::new(3.0_f32, egui::Color32::from_rgb(235, 225, 215)));
    p.line_segment([p0, p1], egui::Stroke::new(1.6_f32, accent));
    p.circle_filled(p1, 3.0, egui::Color32::from_rgb(235, 225, 215));

    // Labels / readout
    let readout_y = rect.bottom() - style.readout_gap;
    p.text(
        egui::pos2(rect.center().x, readout_y - style.value_font * 0.9),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(style.title_font),
        text_mid,
    );
    p.text(
        egui::pos2(rect.center().x, readout_y),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::monospace(style.value_font),
        text_hot,
    );

    let (ls, lc) = start_angle.sin_cos();
    let (es, ec) = end_angle.sin_cos();
    let lbl_r = radius + 18.0;
    p.text(
        knob_center + egui::vec2(lc, ls) * lbl_r + egui::vec2(-4.0, 6.0),
        egui::Align2::CENTER_CENTER,
        left_end,
        egui::FontId::proportional(style.end_font),
        text_dim,
    );
    p.text(
        knob_center + egui::vec2(ec, es) * lbl_r + egui::vec2(4.0, 6.0),
        egui::Align2::CENTER_CENTER,
        right_end,
        egui::FontId::proportional(style.end_font),
        text_dim,
    );

    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
}

fn huge_decay_knob(ui: &mut egui::Ui, params: &HaellvorbParams, setter: &ParamSetter) {
    let avail = ui.available_size();
    let w = avail.x.clamp(300.0, 560.0);
    let h = (avail.y * 0.62).clamp(300.0, 520.0);
    draw_knob(
        ui,
        &params.rev_decay,
        setter,
        "DECAY",
        "SHORT",
        "LONG",
        |secs| {
            if secs < 1.0 {
                format!("{} MS", (secs * 1000.0).round() as i32)
            } else {
                format!("{:.1} S", secs)
            }
        },
        egui::vec2(w, h),
        KnobStyle {
            radius: 120.0,
            value_font: 28.0,
            title_font: 14.0,
            end_font: 10.0,
            readout_gap: 30.0,
        },
    );
}

fn small_knob<P, F>(
    ui: &mut egui::Ui,
    param: &P,
    setter: &ParamSetter,
    title: &str,
    left_end: &str,
    right_end: &str,
    format_value: F,
) where
    P: Param<Plain = f32>,
    F: Fn(f32) -> String,
{
    draw_knob(
        ui,
        param,
        setter,
        title,
        left_end,
        right_end,
        format_value,
        egui::vec2(150.0, 180.0),
        KnobStyle {
            radius: 46.0,
            value_font: 15.0,
            title_font: 11.0,
            end_font: 9.0,
            readout_gap: 18.0,
        },
    );
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
            load_background(ctx);
        },
        move |ui, setter, _queue, _state| {
            ResizableWindow::new("haellvorb-main")
                .min_size(egui::vec2(DEFAULT_GUI_SIZE.0 as f32, DEFAULT_GUI_SIZE.1 as f32))
                .show(ui, editor_state.as_ref(), |ui| {
                    let content_rect = ui.max_rect();
                    let painter = ui.painter_at(content_rect);
                    if let Some(bg) = get_background(ui.ctx()) {
                        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        painter.image(bg.id(), content_rect, uv, egui::Color32::WHITE);
                        painter.rect_filled(content_rect, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 90));
                    } else {
                        painter.rect_filled(content_rect, 0.0, egui::Color32::from_rgb(7, 8, 10));
                    }

                    let panel = egui::Frame::NONE
                        .fill(egui::Color32::TRANSPARENT)
                        .inner_margin(egui::Margin::symmetric(20, 14));
                    panel.show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(2.0);
                            huge_decay_knob(ui, &params, setter);
                            ui.add_space(12.0);

                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
                                small_knob(ui, &params.rev_mix, setter, "MIX", "DRY", "WET", |v| format!("{} %", (v * 100.0).round() as i32));
                                small_knob(ui, &params.rev_predelay, setter, "PRE-DELAY", "0", "250", |v| format!("{} ms", v.round() as i32));
                                small_knob(ui, &params.rev_damp, setter, "DAMPING", "DARK", "BRIGHT", |v| format!("{} %", (v * 100.0).round() as i32));
                                small_knob(ui, &params.in_gain, setter, "IN GAIN", "-24", "+24", |v| format!("{:+.1} dB", v));
                                small_knob(ui, &params.out_gain, setter, "OUT GAIN", "-24", "+24", |v| format!("{:+.1} dB", v));
                            });
                        });
                    });
                });
        },
    )
}
