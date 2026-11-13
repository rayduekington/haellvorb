//! Haellvorb GUI (egui).

use std::sync::Arc;

use nice_plug_egui::{create_egui_editor, resizable_window::ResizableWindow, widgets, EguiState};

use crate::{HaellvorbParams, DEFAULT_GUI_SIZE};

pub fn create(
    params: Arc<HaellvorbParams>,
    editor_state: Arc<EguiState>,
) -> Option<Box<dyn nice_plug::prelude::Editor>> {
    create_egui_editor(
        editor_state.clone(),
        (),
        Default::default(),
        |_egui_ctx, _queue, _state| {},
        move |ui, setter, _queue, _state| {
            ResizableWindow::new("haellvorb-main")
                .min_size(egui::vec2(DEFAULT_GUI_SIZE.0 as f32, DEFAULT_GUI_SIZE.1 as f32))
                .show(ui, editor_state.as_ref(), |ui| {
                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        ui.heading("WILVÖRB");
                        ui.label(
                            egui::RichText::new(
                                "Algorithmic reverb with random Wilhelm screams (pitch-fit to input)",
                            )
                            .italics(),
                        );
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(6.0);

                        ui.columns(2, |cols| {
                            cols[0].heading("Reverb");
                            cols[0].add(widgets::ParamSlider::for_param(&params.rev_on, setter));
                            cols[0].add(widgets::ParamSlider::for_param(&params.rev_mix, setter));
                            cols[0].add(widgets::ParamSlider::for_param(&params.rev_decay, setter));
                            cols[0].add(widgets::ParamSlider::for_param(&params.rev_predelay, setter));
                            cols[0].add(widgets::ParamSlider::for_param(&params.rev_damp, setter));
                            cols[0].add_space(8.0);
                            cols[0].separator();
                            cols[0].add_space(4.0);
                            cols[0].heading("I/O");
                            cols[0].add(widgets::ParamSlider::for_param(&params.in_gain, setter));
                            cols[0].add(widgets::ParamSlider::for_param(&params.out_gain, setter));

                            cols[1].heading("Wilhelm");
                            cols[1].label(
                                egui::RichText::new(
                                    "Triggers on random, changing level thresholds.\nPlayback rate is pitch-fit to detected input.",
                                )
                                .size(12.0),
                            );
                            cols[1].add_space(6.0);
                            cols[1].add(widgets::ParamSlider::for_param(&params.scream_on, setter));
                            cols[1].add(widgets::ParamSlider::for_param(&params.scream_sens, setter));
                            cols[1].add(widgets::ParamSlider::for_param(&params.scream_change, setter));
                            cols[1].add(widgets::ParamSlider::for_param(&params.scream_gain, setter));
                        });
                    });
                });
        },
    )
}
