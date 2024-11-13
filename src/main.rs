mod screen_capture;

use eframe::egui;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use screen_capture::ScreenCapture;  // Import the ScreenCapture module

pub struct MyApp {
    capture: ScreenCapture,  // Screen capture instance
    current_frame: Option<Vec<u8>>,  // Current frame data to display
    last_update: Instant,         // Time tracking to simulate frame rate control
}

impl Default for MyApp {
    fn default() -> Self {
        let capture = ScreenCapture::new().unwrap();  // Initialize screen capture
        Self {
            capture,
            current_frame: None,
            last_update: Instant::now(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Attempt to receive a frame from the capture thread
        if let Ok(frame_data) = self.capture.rx.try_recv() {
            self.current_frame = Some(frame_data);
        }

        // Simulate frame update at a fixed rate (e.g., 30 FPS)
        if self.last_update.elapsed().as_secs_f32() >= 1.0 / 30.0 {
            self.last_update = Instant::now();
        }

        // Display the captured frame (if available)
        if let Some(current_frame) = &self.current_frame {
            let width = self.capture.width; // Replace with actual width of your captured frame
            let height = self.capture.height; // Replace with actual height of your captured frame
            // Convert the raw frame data to an image format compatible with egui
            let texture = egui::ColorImage::from_rgba_unmultiplied(
                [width, height],  // Example resolution, replace with your capture resolution
                current_frame,
            );
            let image_handle = ctx.load_texture("screen_frame", texture, Default::default());

            egui::CentralPanel::default().show(ctx, |ui| {
                // Get the size of the CentralPanel
                let available_size = ui.available_size();

                // Calculate aspect ratio of the frame
                let aspect_ratio = width as f32 / height as f32;

                // Determine the target size that fits within available space
                let target_size = if available_size.x / available_size.y > aspect_ratio {
                    // If panel is wider than the frame's aspect ratio, fit to height
                    egui::vec2(available_size.y * aspect_ratio, available_size.y)
                } else {
                    // Otherwise, fit to width
                    egui::vec2(available_size.x, available_size.x / aspect_ratio)
                };

                // Use `egui::Image` widget to display the image with the calculated size
                ui.add(egui::Image::new(&image_handle).fit_to_exact_size(target_size));
            });
        }
    }
}

fn main() {
    // Define options for the native window
    let options = eframe::NativeOptions::default();

    // Run the egui application
    let _ = eframe::run_native(
        "UStream",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))), // No Result wrapper needed here
    );
}
