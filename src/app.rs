
use eframe::egui;
use crate::caster::Caster;
use crate::receiver::Receiver;

pub struct UStreamApp {
    mode: String,
    caster: Caster,
    receiver : Receiver
}

impl Default for UStreamApp {
    fn default() -> Self {
        Self {
            mode: "receiver".to_string(),
            caster: Caster::new(),
            receiver : Receiver::new(),
        }
    }
}

impl eframe::App for UStreamApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.columns(2, |columns| {
                    if columns[0].selectable_label(self.mode == "receiver", "Receiver").clicked() {
                        self.mode = "receiver".to_string();
                    }
                    if columns[1].selectable_label(self.mode == "caster", "Caster").clicked() {
                        self.mode = "caster".to_string();
                    }
                });

                ui.add_space(20.0);

                // Render content based on the selected mode
                match self.mode.as_str() {
                    "receiver" => self.receiver.render(ui),
                    "caster" => self.caster.render(ui, ctx),
                    _ => (),
                }
            });
        });
    }
}