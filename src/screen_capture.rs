use scrap::{Capturer, Display};
use std::sync::{mpsc};
use std::thread;
use std::time::Duration;
use std::io::ErrorKind::WouldBlock;

fn convert_bgra_to_rgba(frame: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut rgba_frame = Vec::with_capacity((width * height * 4) as usize);
    for chunk in frame.chunks_exact(4) {
        // Convert BGRA to RGBA
        rgba_frame.push(chunk[2]); // R
        rgba_frame.push(chunk[1]); // G
        rgba_frame.push(chunk[0]); // B
        rgba_frame.push(chunk[3]); // A
    }
    rgba_frame
}

pub struct ScreenCapture {
    pub rx: mpsc::Receiver<Vec<u8>>, 
}

impl ScreenCapture {
    // Constructor that initializes the capture thread and returns the receiver
    pub fn new() -> Result<Self, String> {
        // Create a channel to send video data
        let (tx, rx) = mpsc::channel::<Vec<u8>>();

        thread::spawn(move || {
            // Create a Capturer to capture the screen
            let display = Display::primary().unwrap();
            let mut capturer = Capturer::new(display).unwrap();
            let width = capturer.width();
            let height = capturer.height();
            // Start capturing frames in a loop
            let capture_interval = Duration::from_millis(30);
            loop {
                match capturer.frame() {
                    Ok(frame) => {
                        let rgba_frame = convert_bgra_to_rgba(&frame, width, height);
                        if let Err(e) = tx.send(rgba_frame.to_vec()) {
                            eprintln!("Error sending frame: {}", e);
                            break;
                        }
                    }
                    Err(error) => {
                        if error.kind() != WouldBlock {
                            eprintln!("Error capturing frame: {:?}", error);
                            break;
                        }
                    }
                }

                // Sleep for the capture interval (to control FPS)
                thread::sleep(capture_interval);
            }
        });

        Ok(ScreenCapture {
            rx
        })
    }
}

pub fn get_resolution(frame_data: &[u8]) -> Option<(usize, usize)> {
    let total_pixels = frame_data.len() / 4;
    let common_resolutions = [
        (1280, 720), (1600, 900), (1920, 1080), (2560, 1440), (3840, 2160),
        (5120, 2880), (7680, 4320), (1280, 800), (1440, 900), (1680, 1050),
        (1920, 1200), (2560, 1600), (3840, 2400), (2560, 1080), (3440, 1440),
        (3840, 1600), (5120, 2160), (6880, 2880), (3840, 1080), (5120, 1440),
        (7680, 2160), (640, 480), (800, 600), (1024, 768), (1280, 1024),
        (1600, 1200), (2048, 1536), (1280, 1024), (2160, 1440), (3000, 2000),
        (3200, 2133), (1366, 768), (1536, 864), (1792, 1344), (2048, 1080),
        (2048, 1152), (2048, 2048), (3840, 3840), (4096, 2160), (6016, 3384),
        (7680, 3200), (10240, 4320),
    ];

    // Find a matching resolution
    common_resolutions.iter().find_map(|&(width, height)| {
        if total_pixels == width * height {
            Some((width, height))
        } else {
            None
        }
    })
}