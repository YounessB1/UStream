use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio::io::{AsyncWriteExt};
use tokio::runtime::Runtime;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::net::SocketAddr;
use std::collections::HashSet;
use bytes::Bytes;

// Define a struct to manage the server state
pub struct StreamServer {
    clients: Arc<Mutex<HashSet<SocketAddr>>>, // Track connected clients
    sender: broadcast::Sender<Bytes>,         // Broadcast channel for streaming data
    is_streaming: Arc<Mutex<bool>>,           // Streaming state
    pub runtime: Arc<Runtime>,
    client_count: Arc<AtomicUsize>,
}

impl StreamServer {
    // Create a new server instance
    pub fn new() -> Self {
        // Create a Tokio runtime
        let runtime = Arc::new(Runtime::new().unwrap());
        
        let (sender, _) = broadcast::channel(16); // Buffer size of 16 messages
        let clients = Arc::new(Mutex::new(HashSet::new()));
        let is_streaming = Arc::new(Mutex::new(false));
        let client_count = Arc::new(AtomicUsize::new(0));

        let server = Self {
            clients: Arc::clone(&clients),
            sender: sender.clone(),
            is_streaming: Arc::clone(&is_streaming),
            runtime: Arc::clone(&runtime),
            client_count: Arc::clone(&client_count),
        };

        // Use the runtime to spawn a task that starts the server
        let runtime_clone = Arc::clone(&runtime);
        let client_count_clone = Arc::clone(&client_count);
        runtime.spawn(async move {
            let listener = TcpListener::bind(("0.0.0.0", 9041)).await.unwrap();
            println!("Server started on port 9041");

            loop {
                if let Ok((socket, addr)) = listener.accept().await {
                    println!("Client connected: {}", addr);
                    clients.lock().await.insert(addr);
                    client_count_clone.fetch_add(1, Ordering::SeqCst); 

                    let sender = sender.clone();
                    let clients = Arc::clone(&clients);
                    let client_count = Arc::clone(&client_count_clone);

                    // Spawn a task to handle the client
                    runtime_clone.spawn(async move {
                        Self::handle_client(socket, sender, clients, client_count, addr).await;
                    });
                }
            }
        });

        server
    }

    // Handle an individual client connection
    async fn handle_client(
        mut socket: TcpStream,
        mut receiver: broadcast::Sender<Bytes>,
        clients: Arc<Mutex<HashSet<SocketAddr>>>,
        client_count: Arc<AtomicUsize>,
        addr: SocketAddr,
    ) {
        let mut receiver = receiver.subscribe();
        loop {
            match receiver.recv().await {
                Ok(frame) => {
                    if socket.write_all(&frame).await.is_err() {
                        break;
                    }
                }
                Err(_) => break, // Channel closed, exit loop
            }
        }
        println!("Client disconnected: {}", addr);
        clients.lock().await.remove(&addr);
        client_count.fetch_sub(1, Ordering::SeqCst);
    }

    // Broadcast a frame to all connected clients
    pub async fn broadcast_frame(&self, frame: Vec<u8>) {
        if *self.is_streaming.lock().await {
            let _ = self.sender.send(Bytes::from(frame));
        }
    }

    // Disconnect all clients
    pub async fn disconnect(&self) {
        let mut clients = self.clients.lock().await;
        clients.clear();
        self.client_count.store(0, Ordering::SeqCst);
        println!("All clients disconnected");
    }

    pub fn get_client_count(&self) -> usize {
        self.client_count.load(Ordering::SeqCst)
    }
}