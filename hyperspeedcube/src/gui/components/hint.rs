pub struct HintWidget;
impl HintWidget {
    pub fn show<R>(
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui.add(
                egui::Button::new("?")
                    .small()
                    .rounding(16.0)
                    .min_size(egui::vec2(24.0, 16.0))
                    .fill(egui::Color32::TRANSPARENT),
            );

            if r.hovered() {
                egui::containers::show_tooltip_for(ui.ctx(), unique_id!(), &r.rect, |ui| {
                    // TODO: refactor this constant
                    ui.set_width(super::super::ext::EXPLANATION_TOOLTIP_WIDTH * 2.0);
                    add_contents(ui)
                })
            } else {
                None
            }
        })
    }
}

impl egui::Widget for HintWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        todo!()
    }
}