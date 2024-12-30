use std::io::{self, Read};
use std::net::{TcpStream, SocketAddr};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};
use bincode;
use crate::screen::Frame;

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

    //La funzione start può essere utilizzata per avviare la connessione a un server, passandogli l'indirizzo IP del server.
    pub fn start(&mut self, ip_address: &str) -> Result<(), String> {
        self.shutdown_flag.lock().unwrap().store(false, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel::<Option<Frame>>();
        let receiver = Some(rx);
        

        let address = format!("{}:9041", ip_address);
        let addr: SocketAddr = address.parse().map_err(|_| format!("Invalid IP address: {}", ip_address))?;
        //Una connessione TCP viene stabilita utilizzando TcpStream::connect
        let stream = TcpStream::connect(addr).map_err(|e| format!("Connection failed: {}", e))?;
        println!("Successfully connected to {}", addr);

        let thread_shutdown_flag = Arc::clone(&self.shutdown_flag);

        //Un thread separato si occupa di ricevere i dati dalla connessione TCP
        thread::spawn(move || {
            Self::receive_data(stream, tx, thread_shutdown_flag) 
        });

        self.receiver = receiver;
        Ok(())
    }

    fn receive_data(mut stream: TcpStream, tx: mpsc::Sender<Option<Frame>>, shutdown_flag: Arc<Mutex<AtomicBool>>){
        loop {
            // Check for shutdown signal
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

                    let mut frame_buffer = vec![0u8; frame_size];
                    match stream.read_exact(&mut frame_buffer) {
                        Ok(_) => {
                            match bincode::deserialize::<Frame>(&frame_buffer) {
                                Ok(frame) => {
                                    if tx.send(Some(frame)).is_err() {
                                        println!("Receiver disconnected");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to deserialize frame: {}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error reading frame data: {}", e);
                            if e.kind() == io::ErrorKind::UnexpectedEof {
                                println!("Connection closed by the server.");
                                let _ = tx.send(None);
                            }
                            break;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read frame size: {}", e);
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        println!("Connection closed by the server.");
                        let _ = tx.send(None);
                    }
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