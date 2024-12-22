use eframe::egui;
use crate::screen::{ScreenCapture, Frame, CropValues, crop, blank, available_displays, convert_bgra_to_rgba, encode_to_jpeg};
use crate:: server::Server;
use image;

pub struct Caster {
    displays: Vec<String>,
    capture: Option<ScreenCapture>, // Screen capture instance
    server: Server,
    current_frame: Option<Frame>, // Current frame data to display
    crop: CropValues,
    is_streaming : bool,
    is_blank : bool,
}

impl Caster {
    // Initialize the Caster with a new ScreenCapture instance
    pub fn new() -> Self {
        let capture = None;
        let server = Server::new();
        let crop = CropValues::new(0.0, 0.0, 0.0, 0.0);
        let displays = available_displays();
        Self {
            displays,
            capture,
            server,
            current_frame: None,
            crop,
            is_streaming: false,
            is_blank: false
        }
    }

    // Render method for the Caster mode
    pub fn render(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Caster Mode");
        ui.add_space(20.0);
        // Try to receive a frame from the capture thread
        if let Some(capture) = &mut self.capture {
            if let Some(frame) = capture.receive_frame() {
                
                self.current_frame = Some(frame.clone());
                let mut jpeg_data = Vec::new(); // Buffer per i dati JPEG
                let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut jpeg_data);
                
                if encoder.encode(&frame.data, frame.width, frame.height, image::ColorType::Rgba8).is_ok() {
                    let jpeg_frame = Frame {
                        data: jpeg_data,
                        width: frame.width,
                        height: frame.height,
                    };
                    self.server.broadcast_frame(jpeg_frame, self.is_streaming);
                } else {
                    eprintln!("Errore nella codifica JPEG del frame");
                }
            }
        }
        // display possible screens to capture
        else {
            for (index, name) in self.displays.iter().enumerate() {
                if ui.add(egui::Button::new(name)).clicked() {
                     self.capture = Some(ScreenCapture::new(index).unwrap());
                }
                ui.add_space(10.0);
            }
        }
        // Display the captured frame (if available)
        if let Some(frame) = &self.current_frame {
            ui.columns(4, |columns| {
                let slider_width = columns[0].available_width() / 1.0; // Width of each slider (columns width)
            
                // Left Crop Slider
                columns[0].vertical(|ui| {
                    ui.label("Left");
                    ui.add_sized(
                        [slider_width, 20.0],
                        egui::Slider::new(&mut self.crop.left, 0.0..=100.0),
                    );
                });
            
                // Right Crop Slider
                columns[1].vertical(|ui| {
                    ui.label("Right");
                    ui.add_sized(
                        [slider_width, 20.0],
                        egui::Slider::new(&mut self.crop.right, 0.0..=100.0),
                    );
                });
            
                // Top Crop Slider
                columns[2].vertical(|ui| {
                    ui.label("Top");
                    ui.add_sized(
                        [slider_width, 20.0],
                        egui::Slider::new(&mut self.crop.top, 0.0..=100.0),
                    );
                });
            
                // Bottom Crop Slider
                columns[3].vertical(|ui| {
                    ui.label("Bottom");
                    ui.add_sized(
                        [slider_width, 20.0],
                        egui::Slider::new(&mut self.crop.bottom, 0.0..=100.0),
                    );
                });
            });
            ui.add_space(20.0);

            let width = frame.width as usize;
            let height = frame.height as usize;

            // Convert the raw frame data to an egui-compatible image
            let texture = egui::ColorImage::from_rgba_unmultiplied(
                [width, height],
                &frame.data,
            );
            let image_handle = ctx.load_texture("screen_frame", texture, Default::default());

            // Determine available space and aspect ratio
            let mut available_size = ui.available_size();
            available_size.x -= 10.0;   //larghezza
            available_size.y -= 100.0;  //altezza
            let aspect_ratio = width as f32 / height as f32;

            // Calculate the target size to fit the frame within available space
            let target_size = if available_size.x / available_size.y > aspect_ratio {   //spazio più largo di quello disponibile
                egui::vec2(available_size.y * aspect_ratio, available_size.y)
            } else {    //spazio più alto di quello disponibile
                egui::vec2(available_size.x, available_size.x / aspect_ratio)
            };

            // Display the image
            ui.add(egui::Image::new(&image_handle).fit_to_exact_size(target_size));

            ui.add_space(10.0);

            let client_count = self.server.get_client_count();
            ui.label(format!("Connected Clients: {}", client_count));
    
            ui.add_space(10.0);
    
            ui.columns(3, |columns| {
                // Stream/Pause button with Ctrl+S shortcut in the first column
                let stream_button_text = if self.is_streaming { "Pause (Ctrl + S)" } else { "Stream (Ctrl + S)" };
                let stream_button = columns[0].add(egui::Button::new(stream_button_text).fill(egui::Color32::GREEN));
                if stream_button.clicked() || (ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S))) {
                    self.is_streaming = !self.is_streaming; //flag invertito
                }
    
                // Blank/Stop Blank button with Ctrl+B shortcut in the second column
                let blank_button_text = if self.is_blank { "Stop Blank (Ctrl + B)" } else { "Blank (Ctrl + B)" };
                let blank_button = columns[1].button(blank_button_text);
                if blank_button.clicked() || (ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::B))) {
                    self.is_blank = !self.is_blank; //flag invertito
                }
    
                // Disconnect button with Ctrl+D shortcut in the third column
                let disconnect_button = columns[2].add(egui::Button::new("Disconnect (Ctrl + D)").fill(egui::Color32::RED));
                if disconnect_button.clicked() || (ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::D))) {
                    self.is_streaming = false;  
                    self.server.disconnect();
                }
            });
        }
    }
}