use eframe::egui;
use crate::client;
use tokio::sync::mpsc::Receiver;
use tokio::runtime::Runtime;
use std::sync::Arc;

pub struct Receiver {
    ip_address: String,
    connected: bool,
    error_message: Option<String>,
    disconnect_handle: Option<client::DisconnectHandle>,
    runtime: Arc<Runtime>,
    frame_receiver: Option<Receiver<Vec<u8>>>,
    current_frame: Optio<Vec<u8>>,
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
            current_frame: 
            width: 0,
            height: 0
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
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
                while let Ok(frame) = frame_rx.try_recv() {
                    ui.label(format!("Received frame of size: {}", frame.len()));
                }
            }
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