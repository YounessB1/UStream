use std::net::{TcpListener, TcpStream, SocketAddr}; 
use std::io::{BufWriter, Write}; 
use std::sync::{Arc,Mutex,mpsc,atomic::{AtomicUsize,AtomicBool,Ordering}};
use std::collections::{HashMap};
use std::time::{Instant,Duration};
use bytes::{Bytes};
use image::codecs::jpeg::JpegEncoder;
use image::{buffer, ImageBuffer};
use std::thread;
//use bincode;
use crate::screen::Frame;


// Define a struct to manage the server state
pub struct Server {
    sockets: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>,  // Shared map of sockets
    client_count: Arc<AtomicUsize>,                      // Number of connected clients
    time: Instant,                                      // Timing information
    priority: AtomicBool,                              // Priority flag
    sender_tx: mpsc::Sender<Bytes>,                     //canale di invio
}

// ffmpeg, gstream 
impl Server {
    pub fn new() -> Self {
        let sockets = Arc::new(Mutex::new(HashMap::new())); //Mappa che associa un indirizzo a un flusso TCP. Essa è protetta da un mutex per consentire l'accesso concorrente e viene avvolta in un Arc per la condivisione sicura tra thread.
        let client_count = Arc::new(AtomicUsize::new(0));   //contatore atomico
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
            Self::broadcast_thread(sender_rx, sockets_clone, client_count_clone);   //gestisce la trasmissione dei dati ricevuti attraverso il canale
        });

        let sockets_clone = Arc::clone(&sockets);
        let client_count_clone = Arc::clone(&client_count);

        thread::spawn(move || {
            let listener = match TcpListener::bind("0.0.0.0:9041") {    //gestisce le connessioni in arrivo
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
    
                    for (addr, socket) in sockets_lock.iter_mut() {
                        // Wrap the socket in a buffered writer
                        let mut buffered_socket = BufWriter::new(socket);
                        
                        // Attempt to write the message
                        if buffered_socket.write_all(&msg).is_err() {
                            disconnected_clients.push(*addr); // Mark client as disconnected
                        }
                        
                        // Flush to ensure all data is sent
                        if buffered_socket.flush().is_err() {
                            disconnected_clients.push(*addr); // Handle failed flush
                        }
                    }
    
                    // Remove disconnected clients
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
            // Supponendo che il frame sia un buffer RGBA o similare
            let width = frame.width; // Aggiungi width e height alla struttura Frame
            let height = frame.height;
            let raw_data = &frame.data; // Contiene i dati grezzi
    
            // Crea un buffer di immagine da raw_data
            let buffer: ImageBuffer<image::Rgba<u8>, Vec<u8>>  = match ImageBuffer::from_raw(width, height, raw_data.clone()) {
                Some(buffer) => buffer,
                None => {
                    eprintln!("Failed to create image buffer");
                    return None;
                }
            };
    
            // Codifica il buffer in JPEG
            let mut jpeg_data = Vec::new();
            let mut encoder = JpegEncoder::new(&mut jpeg_data);
            if encoder.encode_image(&buffer).is_err() {
                eprintln!("Failed to encode frame as JPEG");
                return None;
            }
    
            // Prepara il messaggio con la dimensione del frame + dati JPEG
            let frame_size = (jpeg_data.len() as u32).to_be_bytes();
            let mut buffer = Vec::with_capacity(4 + jpeg_data.len());
            buffer.extend_from_slice(&frame_size); // Aggiungi dimensione frame
            buffer.extend_from_slice(&jpeg_data);  // Aggiungi dati JPEG
    
            Some(Bytes::from(buffer))
        } else {
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