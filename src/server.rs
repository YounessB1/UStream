use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio::io::{AsyncWriteExt};
use tokio::runtime::Runtime;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::net::SocketAddr;
use std::collections::{HashSet,HashMap};
use bytes::{Bytes,BufMut};
use futures::future::join_all;
use crate::screen::Frame;
use bincode;

// Define a struct to manage the server state
pub struct StreamServer {
    sockets: Arc<Mutex<HashMap<SocketAddr, Arc<Mutex<TcpStream>>>>>, // Updated type
    sender: broadcast::Sender<Bytes>,                               // Broadcast channel
    pub runtime: Arc<Runtime>,
    pub client_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl StreamServer {
    // Create a new server instance
    pub fn new() -> Self {
        // Create a Tokio runtime
        let runtime = Arc::new(Runtime::new().unwrap());
        let (sender, _) = broadcast::channel(256); // Buffer size of 256 messages
        let sockets = Arc::new(Mutex::new(HashMap::new()));
        let client_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let server = Self {
            sockets: Arc::clone(&sockets),
            sender: sender.clone(),
            runtime: Arc::clone(&runtime),
            client_count: Arc::clone(&client_count),
        };

        // Use the runtime to spawn a task that starts the server
        let runtime_clone = Arc::clone(&runtime);
        let sockets_clone = Arc::clone(&sockets);
        let client_count_clone = Arc::clone(&client_count);

        runtime.spawn(async move {
            let listener = match TcpListener::bind(("0.0.0.0", 9041)).await {
                Ok(listener) => listener,
                Err(e) => {
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
                    client_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

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
        mut receiver: broadcast::Sender<Bytes>,
        sockets: Arc<Mutex<HashMap<SocketAddr, Arc<Mutex<TcpStream>>>>>,
        client_count: Arc<std::sync::atomic::AtomicUsize>,
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
        client_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    // Broadcast a frame to all connected clients
    pub async fn broadcast_frame(&self, frame: Frame) {
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

    // Disconnect all clients
    pub async fn disconnect(&self) {
        let mut sockets = self.sockets.lock().await;
        let addr_list: Vec<SocketAddr> = sockets.keys().cloned().collect();
    
        // Create a list of tasks to shutdown sockets concurrently
        let shutdown_tasks: Vec<_> = addr_list.into_iter().map(|addr| {
            let socket = sockets.remove(&addr);
            async move {
                if let Some(socket) = socket {
                    let mut socket = socket.lock().await;
                    if let Err(e) = socket.shutdown().await {
                        eprintln!("Failed to close socket {}: {}", addr, e);
                    }
                }
            }
        }).collect();
    
        // Await all shutdown tasks in parallel
        futures::future::join_all(shutdown_tasks).await;
    
        // Reset the client count
        self.client_count.store(0, std::sync::atomic::Ordering::SeqCst);
        println!("All clients disconnected and sockets closed");
    }

    pub fn get_client_count(&self) -> usize {
        self.client_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}