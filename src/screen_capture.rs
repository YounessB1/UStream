use scrap::{Capturer, Display};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
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
    pub rx: mpsc::Receiver<Vec<u8>>,  // Receiver for video data (H.264 encoded)
    capture_thread: Option<thread::JoinHandle<()>>,  // Handle to join the capture thread
    stop_flag: Arc<AtomicBool>, // Flag to stop the capture thread
    pub width: usize,
    pub height: usize  
}

impl ScreenCapture {
    // Constructor that initializes the capture thread and returns the receiver
    pub fn new() -> Result<Self, String> {
        // Create a channel to send video data
        let (tx, rx) = mpsc::channel::<Vec<u8>>();

        let display = Display::primary().unwrap();
        let temp_capturer = Capturer::new(display).unwrap();
        let width = temp_capturer.width();
        let height = temp_capturer.height();

        // Flag to stop the background thread
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Spawn the background thread for screen capture and video encoding
        let stop_flag_clone = Arc::clone(&stop_flag);
        let capture_thread = Some(thread::spawn(move || {
            // Create a Capturer to capture the screen
            let display = Display::primary().unwrap();
            let mut capturer = Capturer::new(display).unwrap();
            let width = capturer.width();
            let height = capturer.height();
            // Start capturing frames in a loop
            let capture_interval = Duration::from_millis(30);
            while !stop_flag_clone.load(Ordering::SeqCst) {
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

            // Inform that the capture thread is stopping
            println!("Capture thread stopped.");
        }));

        Ok(ScreenCapture {
            rx,
            capture_thread,
            stop_flag,
            width,
            height
        })
    }

    // Method to stop the capture thread
    pub fn stop_capture(&mut self) {
        // Set the stop flag to true, which will stop the capture thread
        self.stop_flag.store(true, Ordering::SeqCst);

        // Join the thread to make sure it finishes cleanly
        if let Some(handle) = self.capture_thread.take() {
            handle.join().unwrap();
            println!("Capture thread joined.");
        }
    }
}