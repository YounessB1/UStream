use eframe::egui;
use crate::screen_capture::{ScreenCapture, get_resolution};
use crate::crop_blank::{crop,blank};
use crate:: server::StreamServer;
use std::sync::Arc;

pub struct Caster {
    capture: ScreenCapture,        // Screen capture instance
    server: StreamServer,
    current_frame: Option<Vec<u8>>, // Current frame data to display
    width: usize,
    height: usize,
    crop: CropValues,
    is_streaming : bool,
    is_blank : bool,
}

pub struct CropValues {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl Caster {
    // Initialize the Caster with a new ScreenCapture instance
    pub fn new() -> Self {
        let capture = ScreenCapture::new().unwrap();
        let server = StreamServer::new();
        Self {
            capture,
            server,
            current_frame: None,
            width: 0,
            height: 0,
            crop: CropValues {
                left: 0.0,
                right: 0.0,
                top: 0.0,
                bottom: 0.0,
            },
            is_streaming: false,
            is_blank: false
        }
    }

    // Render method for the Caster mode
    pub fn render(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Caster Mode");
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let slider_width = ui.available_width() / 4.0;
    
            // Left Crop Slider
            ui.vertical(|ui| {
                ui.label("Left");
                ui.add_sized(
                    [slider_width, 20.0],
                    egui::Slider::new(&mut self.crop.left, 0.0..=100.0),
                );
            });
    
            // Right Crop Slider
            ui.vertical(|ui| {
                ui.label("Right");
                ui.add_sized(
                    [slider_width, 20.0],
                    egui::Slider::new(&mut self.crop.right, 0.0..=100.0),
                );
            });
    
            // Top Crop Slider
            ui.vertical(|ui| {
                ui.label("Top");
                ui.add_sized(
                    [slider_width, 20.0],
                    egui::Slider::new(&mut self.crop.top, 0.0..=100.0),
                );
            });
    
            // Bottom Crop Slider
            ui.vertical(|ui| {
                ui.label("Bottom");
                ui.add_sized(
                    [slider_width, 20.0],
                    egui::Slider::new(&mut self.crop.bottom, 0.0..=100.0),
                );
            });
        });
        ui.add_space(20.0);
        // Try to receive a frame from the capture thread
        if let Ok(frame_data) = self.capture.rx.try_recv() {
            self.current_frame = Some(frame_data);
            crop(
                &mut self.current_frame.as_mut().unwrap(),
                self.width,
                self.height,
                self.crop.left,
                self.crop.right,
                self.crop.top,
                self.crop.bottom,
            );
            blank( &mut self.current_frame.as_mut().unwrap(),self.is_blank);
        }
        // send frame
        let runtime = Arc::clone(&self.server.runtime);
        if self.is_streaming{
            let frame = self.current_frame.clone();
            if let Some(frame) = frame {
                runtime.block_on(async {
                    self.server.broadcast_frame(frame).await
                });
            }
        }
        // Display the captured frame (if available)
        if let Some(frame_data) = &self.current_frame {
            if self.width==0 || self.height==0 {
                if let Some((width, height)) = get_resolution(frame_data) {
                    self.width = width;
                    self.height = height;
                }
            }
            let width = self.width;
            let height = self.height;

            // Convert the raw frame data to an egui-compatible image
            let texture = egui::ColorImage::from_rgba_unmultiplied(
                [width, height],
                frame_data,
            );
            let image_handle = ctx.load_texture("screen_frame", texture, Default::default());

            // Determine available space and aspect ratio
            let mut available_size = ui.available_size();
            available_size.x -= 10.0;
            available_size.y -= 80.0;
            let aspect_ratio = width as f32 / height as f32;

            // Calculate the target size to fit the frame within available space
            let target_size = if available_size.x / available_size.y > aspect_ratio {
                egui::vec2(available_size.y * aspect_ratio, available_size.y)
            } else {
                egui::vec2(available_size.x, available_size.x / aspect_ratio)
            };

            // Display the image
            ui.add(egui::Image::new(&image_handle).fit_to_exact_size(target_size));
        } else {
            ui.label("No frame available.");
        }

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            // Display the number of connected clients
            let client_count = self.server.get_client_count();
            ui.label(format!("Connected Clients: {}", client_count));
        });
        
        // Add some spacing between the label and the buttons
        ui.add_space(10.0);


        ui.horizontal_centered(|ui| {  // This horizontally centers the buttons
            // Stream/Pause button
            let stream_button_text = if self.is_streaming { "Pause" } else { "Stream" };
            if ui.add_sized([120.0, 30.0], egui::Button::new(stream_button_text)).clicked() {
                self.is_streaming= !self.is_streaming;
            }
    
            // Add space between buttons
            ui.add_space(10.0); // Adjust the space between buttons as needed
    
            // Blank/Stop Blank button
            let blank_button_text = if self.is_blank { "Stop Blank" } else { "Blank" };
            if ui.add_sized([120.0, 30.0], egui::Button::new(blank_button_text)).clicked() {
                self.is_blank = !self.is_blank;
            }
    
            // Add space between buttons
            ui.add_space(10.0); // Adjust the space between buttons as needed
    
            // Disconnect button
            if ui.add_sized([120.0, 30.0], egui::Button::new("Disconnect")).clicked() {
                let runtime = Arc::clone(&self.server.runtime);
                runtime.block_on(async {
                    self.server.disconnect().await;
                });
            }
        });
    }
}