mod app;
mod receiver;
mod caster;
mod screen;
mod client;
mod server;

fn main() {
    let app = app::UStreamApp::default();
    // Run the egui application
    let _ = eframe::run_native(
        "UStream",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app))), // No Result wrapper needed here
    );
}
