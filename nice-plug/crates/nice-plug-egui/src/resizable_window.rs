//! Resizable window wrapper for Egui editor.

use egui::emath::GuiRounding;
use egui::{CentralPanel, Id, Rect, Response, Sense, Ui, Vec2, pos2};
use egui::{InnerResponse, UiBuilder};

use crate::EguiState;

/// Adds a corner to the plugin window that can be dragged in order to resize it.
/// Resizing happens through plugin API, hence a custom implementation is needed.
pub struct ResizableWindow {
    id: Id,
    min_size: Vec2,
    aspect_ratio: Option<f32>,
}

impl ResizableWindow {
    pub fn new(id_source: impl std::hash::Hash) -> Self {
        Self {
            id: Id::new(id_source),
            min_size: Vec2::splat(16.0),
            aspect_ratio: None,
        }
    }

    /// Won't shrink to smaller than this
    #[inline]
    pub fn min_size(mut self, min_size: impl Into<Vec2>) -> Self {
        self.min_size = min_size.into();
        self
    }

    /// If set, resizing will preserve the given width/height ratio.
    #[inline]
    pub fn aspect_ratio(mut self, aspect_ratio: f32) -> Self {
        self.aspect_ratio = Some(aspect_ratio);
        self
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        egui_state: &EguiState,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R> {
        CentralPanel::default().show_inside(ui, move |ui| {
            let ui_rect = ui.clip_rect();
            let mut content_ui =
                ui.new_child(UiBuilder::new().max_rect(ui_rect).layout(*ui.layout()));

            let ret = add_contents(&mut content_ui);

            let corner_size = Vec2::splat(ui.visuals().resize_corner_size);
            let corner_rect = Rect::from_min_size(ui_rect.max - corner_size, corner_size);

            let corner_response = ui.interact(corner_rect, self.id.with("corner"), Sense::drag());

            if let Some(pointer_pos) = corner_response.interact_pointer_pos() {
                let mut desired_size = (pointer_pos - ui_rect.min + 0.5 * corner_response.rect.size())
                    .max(self.min_size);

                if let Some(ratio) = self.aspect_ratio {
                    if ratio.is_finite() && ratio > 0.0 {
                        // Lock to aspect ratio: choose size on the ratio line closest to drag intent.
                        let w_from_h = desired_size.y * ratio;
                        let h_from_w = desired_size.x / ratio;
                        if (desired_size.x - w_from_h).abs() <= (desired_size.y - h_from_w).abs() {
                            desired_size.x = w_from_h;
                        } else {
                            desired_size.y = h_from_w;
                        }
                        desired_size = desired_size.max(self.min_size);
                        // Re-enforce ratio after min clamp
                        if desired_size.x / ratio < desired_size.y {
                            desired_size.y = desired_size.x / ratio;
                        } else {
                            desired_size.x = desired_size.y * ratio;
                        }
                    }
                }

                if corner_response.dragged() {
                    egui_state.set_requested_size((
                        desired_size.x.round().max(1.0) as u32,
                        desired_size.y.round().max(1.0) as u32,
                    ));
                }
            }

            paint_resize_corner(&content_ui, &corner_response);

            ret
        })
    }
}

pub fn paint_resize_corner(ui: &Ui, response: &Response) {
    let stroke = ui.style().interact(response).fg_stroke;

    let painter = ui.painter();
    let rect = response.rect.translate(-Vec2::splat(2.0)); // move away from the corner
    let cp = rect.max.round_to_pixels(painter.pixels_per_point());

    let mut w = 2.0;

    while w <= rect.width() && w <= rect.height() {
        painter.line_segment([pos2(cp.x - w, cp.y), pos2(cp.x, cp.y - w)], stroke);
        w += 4.0;
    }
}
