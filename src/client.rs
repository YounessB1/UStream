use std::io::{self,BufReader, Read};
use std::net::{TcpStream, SocketAddr};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};
//use bincode;
use crate::screen::Frame;
use image::io::Reader as ImageReader;
use std::io::Cursor;

pub struct Client {
    pub receiver: Option<mpsc::Receiver<Option<Frame>>>,
    shutdown_flag: Arc<Mutex<AtomicBool>>,
}

impl Client {
    pub fn new() -> Self {
        Self {
            receiver: None,
            shutdown_flag: Arc::new(Mutex::new(AtomicBool::new(false))),
        }
    }

    pub fn start(&mut self, ip_address: &str) -> Result<(), String> {
        self.shutdown_flag.lock().unwrap().store(false, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel::<Option<Frame>>();
        let receiver = Some(rx);
        

        let address = format!("{}:9041", ip_address);
        let addr: SocketAddr = address.parse().map_err(|_| format!("Invalid IP address: {}", ip_address))?;
        let stream = TcpStream::connect(addr).map_err(|e| format!("Connection failed: {}", e))?;
        println!("Successfully connected to {}", addr);

        let thread_shutdown_flag = Arc::clone(&self.shutdown_flag);
        thread::spawn(move || {
            Self::receive_data(stream, tx, thread_shutdown_flag) 
        });

        self.receiver = receiver;
        Ok(())
    }

    fn receive_data(s: TcpStream, tx: mpsc::Sender<Option<Frame>>, shutdown_flag: Arc<Mutex<AtomicBool>>){
        let mut stream = BufReader::new(s);
        loop {
            if shutdown_flag.lock().unwrap().load(Ordering::SeqCst) {
                println!("Shutting down the client...");
                break;
            }

            let mut size_buffer = [0u8; 4];
            match stream.read_exact(&mut size_buffer) {
                Ok(_) => {
                    let frame_size = u32::from_be_bytes(size_buffer) as usize;
                    if frame_size == 0 {
                        continue;
                    }

                    let mut jpeg_data = vec![0u8; frame_size];
                    match stream.read_exact(&mut jpeg_data) {
                        Ok(_) => {
                            // Decodifica i dati JPEG
                            let cursor = Cursor::new(jpeg_data);
                            match ImageReader::new(cursor).with_guessed_format() {
                                Ok(reader) => match reader.decode() {
                                    Ok(image) => {
                                        let frame = Frame {
                                            width: image.width(),
                                            height: image.height(),
                                            data: image.to_rgba8().to_vec(),
                                        };
                                        if tx.send(Some(frame)).is_err() {
                                            println!("Receiver disconnected");
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to decode JPEG: {}", e);
                                        break;
                                    }
                                },
                                Err(e) => {
                                    eprintln!("Failed to create image reader: {}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error reading JPEG data: {}", e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read frame size: {}", e);
                    break;
                }
            }
        }
    }

    pub fn stop(&self) {
        // Set the shutdown flag to true to stop the receiving thread
        self.shutdown_flag.lock().unwrap().store(true, Ordering::SeqCst);
    }
}