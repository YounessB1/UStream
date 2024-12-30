
use eframe::egui;
use crate::caster::Caster;
use crate::receiver::Receiver;

//Struct che mi dice se siamo in modalita' Caster/Receiver
pub struct UStreamApp {
    mode: String,
    caster: Caster,
    receiver : Receiver
}

impl Default for UStreamApp {
    fn default() -> Self { //creo una istanza di Caster/Receiver
        Self {
            mode: "receiver".to_string(),
            caster: Caster::new(),
            receiver : Receiver::new(),
        }
    }
}

impl eframe::App for UStreamApp {
    //Rust a differenza di React aggiorna sempre tutta la pagina ogni secondo e 
    //non solo un componente quindi 
    //questa funzione di update dice cosa aggiornare ogni volta
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.columns(2, |columns| { //seleziono la modalita'
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
                    "receiver" => self.receiver.render(ui, ctx),
                    "caster" => self.caster.render(ui, ctx),
                    _ => (),
                }
            });
        });
    }
}