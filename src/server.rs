use std::net::{TcpListener, TcpStream, SocketAddr}; 
use std::io::{Write}; 
use std::sync::{Arc,Mutex,mpsc,atomic::{AtomicUsize,AtomicBool,Ordering}};
use std::collections::{HashMap};
use std::time::{Instant,Duration};
use bytes::{Bytes};
use std::thread;
use bincode;
use crate::screen::Frame;


// Define a struct to manage the server state
pub struct Server {
    sockets: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>,  // Shared map of sockets
    client_count: Arc<AtomicUsize>,                      // Number of connected clients
    time: Instant,                                      // Timing information
    priority: AtomicBool,                              // Priority flag
    sender_tx: mpsc::Sender<Bytes>, 
}

impl Server {
    pub fn new() -> Self {
        let sockets = Arc::new(Mutex::new(HashMap::new()));
        let client_count = Arc::new(AtomicUsize::new(0));
        let (sender_tx, sender_rx) = mpsc::channel::<Bytes>();

        let server = Self {
            sockets: Arc::clone(&sockets),
            client_count: Arc::clone(&client_count),
            time: Instant::now(),
            priority: AtomicBool::new(false),
            sender_tx
        };
        let sockets_clone = Arc::clone(&sockets);
        let client_count_clone = Arc::clone(&client_count);
        
        thread::spawn(move || {
            Self::broadcast_thread(sender_rx, sockets_clone, client_count_clone);
        });

        let sockets_clone = Arc::clone(&sockets);
        let client_count_clone = Arc::clone(&client_count);

        thread::spawn(move || {
            let listener = match TcpListener::bind("0.0.0.0:9041") {
                Ok(listener) => listener,
                Err(_) => {
                    return;
                }
            };
            println!("Server started on port 9041");

            for stream in listener.incoming() {
                match stream {
                    Ok(socket) => {
                        let addr = socket.peer_addr().unwrap();
                        println!("Client connected: {}", addr);

                        let sockets = Arc::clone(&sockets_clone);
                        let client_count = Arc::clone(&client_count_clone);

                        {
                            let mut sockets_guard = sockets.lock().unwrap();
                            sockets_guard.insert(addr, socket.try_clone().expect("Failed to clone socket"));
                        }
                        client_count.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => {
                        eprintln!("Connection failed: {}", e);
                    }
                }
            }
        });

        server
    }

    fn broadcast_thread(receiver: mpsc::Receiver<Bytes>, sockets: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>, client_count: Arc<AtomicUsize>){
        loop{
            match receiver.recv() {
                Ok(msg) => {
                    let mut sockets_lock = sockets.lock().unwrap();
                    let mut disconnected_clients = Vec::new();

                    // Iterate through the connected sockets and send the message
                    for (addr, socket) in sockets_lock.iter_mut() {
                        if socket.write_all(&msg).is_err() {
                            disconnected_clients.push(*addr);
                        }
                    }

                    // Remove disconnected clients and update client count
                    for addr in disconnected_clients {
                        sockets_lock.remove(&addr);
                        client_count.fetch_sub(1, Ordering::SeqCst);
                        println!("Client {} disconnected", addr);
                    }
                }
                Err(_) => {
                    // Receiver channel is disconnected, should be handled here if needed
                    break;
                }
            }
        }
    }

    fn construct_message(frame: &Frame, is_streaming: bool) -> Option<Bytes> {
        if is_streaming {
            // Serialize the frame to bytes
            let serialized_frame = match bincode::serialize(&frame) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Failed to serialize frame: {}", e);
                    return None;
                }
            };
    
            // Frame size (4 bytes)
            let frame_size = (serialized_frame.len() as u32).to_be_bytes();
            
            // Prepare the buffer with size + serialized data
            let mut buffer = Vec::with_capacity(4 + serialized_frame.len());
            buffer.extend_from_slice(&frame_size);      // Frame size
            buffer.extend_from_slice(&serialized_frame); // Serialized data
            
            Some(Bytes::from(buffer))
        } else {
            // If not streaming, just send a size prefix of 0
            let frame_size = (0 as u32).to_be_bytes();
            let buffer = vec![frame_size[0], frame_size[1], frame_size[2], frame_size[3]];
            Some(Bytes::from(buffer))
        }
    }

    // Broadcast a frame to all connected clients
    pub fn broadcast_frame(&mut self, frame: Frame, is_streaming: bool) {
        if self.priority.load(Ordering::SeqCst) {
            return;
        }
        let message = match Self::construct_message(&frame, is_streaming) {
            Some(msg) => msg,
            None => return, 
        };
        let now = Instant::now();
        if now.duration_since(self.time) >= Duration::from_millis(60) {
            _ = self.sender_tx.send(message);
            self.time = now;
        }
    }

    // Disconnect all clients
    pub fn disconnect(&self) {
        self.priority.store(true, Ordering::SeqCst);
        let mut sockets = self.sockets.lock().unwrap();
        let addr_list: Vec<SocketAddr> = sockets.keys().cloned().collect();

        for addr in addr_list {
            if let Some(socket) = sockets.remove(&addr) {
                if let Err(e) = socket.shutdown(std::net::Shutdown::Both) {
                    eprintln!("Failed to close socket {}: {}", addr, e);
                }
            }
        }
    
        // Reset the client count
        self.client_count.store(0,Ordering::SeqCst);
        self.priority.store(false, Ordering::SeqCst);
        println!("All clients disconnected and sockets closed");
    }

    pub fn get_client_count(&self) -> usize {
        self.client_count.load(Ordering::SeqCst)
    }
}