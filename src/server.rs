use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio::io::{AsyncWriteExt};
use tokio::runtime::Runtime;
use std::sync::{Arc, atomic::{AtomicUsize,AtomicBool,Ordering}};
use std::net::SocketAddr;
use std::collections::{HashMap};
use bytes::{Bytes};
use std::time::{Instant,Duration};
use crate::screen::Frame;
use bincode;

// Define a struct to manage the server state
pub struct StreamServer {
    sockets: Arc<Mutex<HashMap<SocketAddr, Arc<Mutex<TcpStream>>>>>, // Updated type
    sender: broadcast::Sender<Bytes>,                               // Broadcast channel
    runtime: Arc<Runtime>,
    client_count: Arc<AtomicUsize>,
    time: Instant,
    priority: AtomicBool,
}

impl StreamServer {
    // Create a new server instance
    pub fn new() -> Self {
        // Create a Tokio runtime
        let runtime = Arc::new(Runtime::new().unwrap());
        let (sender, _) = broadcast::channel(2048); // Buffer size of 256 messages
        let sockets = Arc::new(Mutex::new(HashMap::new()));
        let client_count = Arc::new(AtomicUsize::new(0));

        let server = Self {
            sockets: Arc::clone(&sockets),
            sender: sender.clone(),
            runtime: Arc::clone(&runtime),
            client_count: Arc::clone(&client_count),
            time: Instant::now(),
            priority: AtomicBool::new(false),
        };

        // Use the runtime to spawn a task that starts the server
        let runtime_clone = Arc::clone(&runtime);
        let sockets_clone = Arc::clone(&sockets);
        let client_count_clone = Arc::clone(&client_count);

        runtime.spawn(async move {
            let listener = match TcpListener::bind(("0.0.0.0", 9041)).await {
                Ok(listener) => listener,
                Err(_) => {
                    return; // Exit the task if binding fails
                }
            };
            println!("Server started on port 9041");

            loop {
                if let Ok((socket, addr)) = listener.accept().await {
                    println!("Client connected: {}", addr);
                    let socket_arc = Arc::new(Mutex::new(socket));

                    // Add the new socket to the sockets map
                    sockets_clone.lock().await.insert(addr, Arc::clone(&socket_arc));
                    client_count_clone.fetch_add(1,Ordering::SeqCst);

                    let sender = sender.clone();
                    let sockets = Arc::clone(&sockets_clone);
                    let client_count = Arc::clone(&client_count_clone);

                    // Spawn a task to handle the client
                    runtime_clone.spawn(async move {
                        Self::handle_client(socket_arc, sender, sockets, client_count, addr).await;
                    });
                }
            }
        });

        server
    }

    // Handle an individual client connection
    async fn handle_client(
        socket: Arc<Mutex<TcpStream>>, // Wrapped in Arc<Mutex<>>
        receiver: broadcast::Sender<Bytes>,
        sockets: Arc<Mutex<HashMap<SocketAddr, Arc<Mutex<TcpStream>>>>>,
        client_count: Arc<AtomicUsize>,
        addr: SocketAddr,
    ) {
        let mut receiver = receiver.clone().subscribe();

        loop {
            match receiver.recv().await {
                Ok(frame) => {
                    let mut socket = socket.lock().await;
                    if socket.write_all(&frame).await.is_err() {
                        break;
                    }
                }
                Err(_) => break, // Channel closed
            }
        }

        println!("Client disconnected: {}", addr);
        sockets.lock().await.remove(&addr);
        let mut current_value = client_count.load(Ordering::SeqCst);
        while current_value > 0 {
            let new_value = current_value - 1;
            if let Ok(_) = client_count.compare_exchange(current_value, new_value, Ordering::SeqCst, Ordering::SeqCst) {
                break;  
            }
            current_value = client_count.load(Ordering::SeqCst);
        }
    }

    // Broadcast a frame to all connected clients
    pub fn broadcast_frame(&mut self, frame: Frame, is_streaming:bool) {
        if self.priority.load(Ordering::SeqCst) {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.time) >= Duration::from_millis(60){
            if is_streaming{
                let serialized_frame = match bincode::serialize(&frame) {
                    Ok(data) => data,
                    Err(e) => {
                        eprintln!("Failed to serialize frame: {}", e);
                        return;
                    }
                };
                let frame_size = (serialized_frame.len() as u32).to_be_bytes();
        
                // Prepare the buffer with size + serialized data
                let mut buffer = Vec::with_capacity(4 + serialized_frame.len());
                buffer.extend_from_slice(&frame_size);      // Frame size (4 bytes)
                buffer.extend_from_slice(&serialized_frame);     // Add the frame data
        
                let _ = self.sender.send(Bytes::from(buffer));
            }
            else {
                // Send only the size prefix of 0 (4 bytes)
                let frame_size = (0 as u32).to_be_bytes();
                let mut buffer = Vec::with_capacity(4);
                buffer.extend_from_slice(&frame_size); 
    
                let _ = self.sender.send(Bytes::from(buffer));
            }
            self.time = now;
        }

    }

    // Disconnect all clients
    pub fn disconnect(&self) {
        self.priority.store(true, Ordering::SeqCst);
        self.runtime.block_on(async {
            let mut sockets = self.sockets.lock().await;
            let addr_list: Vec<SocketAddr> = sockets.keys().cloned().collect();
        
            // Iterate over each socket and perform shutdown synchronously
            for addr in addr_list {
                if let Some(socket) = sockets.remove(&addr) {
                    let mut socket = socket.lock().await;
                    if let Err(e) = socket.shutdown().await {
                        eprintln!("Failed to close socket {}: {}", addr, e);
                    }
                }
            }
        });
    
        // Reset the client count
        self.client_count.store(0,Ordering::SeqCst);
        self.priority.store(false, Ordering::SeqCst);
        println!("All clients disconnected and sockets closed");
    }

    pub fn get_client_count(&self) -> usize {
        self.client_count.load(Ordering::SeqCst)
    }
}