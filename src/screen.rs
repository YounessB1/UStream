use scrap::{Capturer, Display};
use winit::event_loop::EventLoop;
use winit::monitor::MonitorHandle;
use std::sync::{mpsc};
use std::thread;
use std::time::Duration;
use std::io::ErrorKind::WouldBlock;
use tokio::sync::watch;
use serde::{Deserialize, Serialize};

fn convert_bgra_to_rgba(frame: &[u8], width: u32, height: u32) -> Vec<u8> {
    let h = height as usize;
    let w = width as usize;
    let stride = frame.len() / h; 
    let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..h {
        for x in 0..w {
            let i = stride * y + 4 * x;
            rgba_data.extend_from_slice(&[
                frame[i + 2],
                frame[i + 1],
                frame[i],
                255,
            ]);
        }
    }

    rgba_data
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Frame{
    pub data: Vec<u8>,
    pub width: u32,
    pub height : u32
}
pub struct ScreenCapture {
    pub rx: watch::Receiver<Frame>,
}

#[derive(Clone)]
pub struct CropValues {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

pub fn available_displays() -> Vec<String> {
    let event_loop = EventLoop::new();
    let displays: Vec<String> = event_loop
        .available_monitors()
        .map(|monitor| monitor.name().unwrap_or_else(|| "Unknown Display".to_string()))
        .collect();

    displays
}

impl ScreenCapture {
    // Constructor that initializes the capture thread and returns the receiver
    pub fn new(index: usize) -> Result<Self, String> {
        let (tx, rx) = watch::channel(Frame {
            data: vec![],
            width: 0,
            height: 0,
        });

        thread::spawn(move || {
            // Create a Capturer to capture the screen
            let mut displays = Display::all().unwrap();
            let display = displays.remove(index);
            let mut capturer = Capturer::new(display).unwrap();
            let width = capturer.width() as u32;
            let height = capturer.height() as u32;

            // Start capturing frames in a loop
            let capture_interval = Duration::from_millis(30);
            loop {
                match capturer.frame() {
                    Ok(frame) => {
                        let rgba_frame = convert_bgra_to_rgba(&frame, width, height);
                        let frame_data = Frame {
                            data: rgba_frame.to_vec(),
                            width,
                            height,
                        };

                        if tx.send(frame_data).is_err() {
                            eprintln!("Receiver has been dropped, stopping capture.");
                            break;
                        }
                    }
                    Err(error) => {
                        if error.kind() != std::io::ErrorKind::WouldBlock {
                            eprintln!("Error capturing frame: {:?}", error);
                            break;
                        }
                    }
                }

                // Sleep for the capture interval (to control FPS)
                thread::sleep(capture_interval);
            }
        });

        Ok(ScreenCapture { rx })
    }

    pub fn receive_frame(&mut self) -> Option<Frame> {
        let frame = self.rx.borrow();
        if !frame.data.is_empty() {
            Some(frame.clone())
        } else {
            None
        }
    }
}

impl CropValues {
    pub fn new(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        Self { left, right, top, bottom }
    }
}

pub fn crop(frame: &mut Frame, crop: CropValues) {
    let channels = 4; // Assuming RGBA format (4 bytes per pixel)
    let width = frame.width as usize;
    let height = frame.height as usize;

    // Calculate the pixel bounds for each side based on percentages
    let left_bound = ((crop.left / 100.0) * width as f32).round() as usize;
    let right_bound = ((crop.right / 100.0) * width as f32).round() as usize;
    let top_bound = ((crop.top / 100.0) * height as f32).round() as usize;
    let bottom_bound = ((crop.bottom / 100.0) * height as f32).round() as usize;

    // Modify the data field of the Frame in-place
    for y in 0..height {
        for x in 0..left_bound {
            let index = (y * width + x) * channels;
            frame.data[index..index + channels].copy_from_slice(&[255, 255, 255, 255]);
        }
    }

    for y in 0..height {
        for x in (width - right_bound)..width {
            let index = (y * width + x) * channels;
            frame.data[index..index + channels].copy_from_slice(&[255, 255, 255, 255]);
        }
    }

    for y in 0..top_bound {
        for x in 0..width {
            let index = (y * width + x) * channels;
            frame.data[index..index + channels].copy_from_slice(&[255, 255, 255, 255]);
        }
    }

    for y in (height - bottom_bound)..height {
        for x in 0..width {
            let index = (y * width + x) * channels;
            frame.data[index..index + channels].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
}

pub fn blank(frame: &mut Frame, is_blank: bool) {
    // Assuming the frame is in RGBA format (4 bytes per pixel)
    if is_blank {
        for chunk in frame.data.chunks_exact_mut(4) {
            chunk.copy_from_slice(&[255, 255, 255, 255]); // Fill with white (RGBA)
        }
    }
}