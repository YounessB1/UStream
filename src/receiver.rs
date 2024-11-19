use eframe::egui;
use crate::client;
use tokio::sync::mpsc;
use tokio::runtime::Runtime;
use std::sync::Arc;
use crate::screen_capture::{ScreenCapture, get_resolution};

pub struct Receiver {
    ip_address: String,
    connected: bool,
    error_message: Option<String>,
    disconnect_handle: Option<client::DisconnectHandle>,
    runtime: Arc<Runtime>,
    frame_receiver: Option<mpsc::Receiver<Vec<u8>>>,
    current_frame: Option<Vec<u8>>,
    width: usize,
    height: usize,
}

impl Receiver {
    pub fn new() -> Self {
        // Initialize a new Tokio runtime for async tasks
        let runtime = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));
        Self {
            ip_address: String::new(),
            connected: false,
            error_message: None,
            disconnect_handle: None,
            runtime,
            frame_receiver: None,
            current_frame: None,
            width: 0,
            height: 0,
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui,ctx: &egui::Context) {
        ui.heading("Receiver Mode");

        // Display the error message if there is one
        if let Some(error) = &self.error_message {
            ui.colored_label(egui::Color32::RED, error);
        }

        // Input field for the IP Address
        ui.horizontal(|ui| {
            if self.connected {
                // Render the disabled input by making it non-editable
                ui.add_enabled(
                    false,
                    egui::TextEdit::singleline(&mut self.ip_address).hint_text("Enter IP Address"),
                );
            } else {
                ui.add(
                    egui::TextEdit::singleline(&mut self.ip_address).hint_text("Enter IP Address"),
                );
            }

            // Button group
            if self.connected {
                if ui
                    .add(egui::Button::new("Disconnect").fill(egui::Color32::RED))
                    .clicked()
                {
                    self.handle_disconnect();
                }
            } else {
                if ui
                    .add(egui::Button::new("Connect").fill(egui::Color32::GREEN))
                    .clicked()
                {
                    self.handle_connect();
                }
            }
        });

        // Display received frames if connected
        if self.connected {
            if let Some(frame_rx) = &mut self.frame_receiver {
                if let Ok(frame) = frame_rx.try_recv() {
                    println!("Received frame");
                    self.current_frame = Some(frame);
                }
            }
        }

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
    }

    fn handle_connect(&mut self) {
        // Clear any previous errors
        self.error_message = None;

        // If the IP address is not empty, try to connect
        if !self.ip_address.is_empty() {
            println!("Connecting to {}", self.ip_address);
            let ip = self.ip_address.clone();
            let runtime = Arc::clone(&self.runtime);

            // Spawn a new async task to handle the connection
            let result = runtime.block_on(async {
                client::connect_to_server(&ip).await
            });

            match result {
                Ok((frame_rx, disconnect_handle)) => {
                    self.connected = true;
                    self.disconnect_handle = Some(disconnect_handle);
                    self.frame_receiver = Some(frame_rx);
                    self.error_message = None;
                }
                Err(err) => {
                    self.error_message = Some(format!("Error: {}", err));
                }
            }
        }
    }

    fn handle_disconnect(&mut self) {
        println!("Disconnected");
        if let Some(handle) = self.disconnect_handle.take() {
            self.runtime.block_on(handle.disconnect());
        }
        self.connected = false;
    }
}