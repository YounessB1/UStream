use std::net::{TcpListener, TcpStream, SocketAddr}; 
use std::io::{Write}; 
use std::sync::{Arc,Mutex,mpsc,atomic::{AtomicUsize,AtomicBool,Ordering}};
use std::collections::{HashMap};
use std::time::{Instant,Duration};
use bytes::{Bytes};
use std::thread;
use bincode;
use image::{DynamicImage, ImageOutputFormat};
use std::io::Cursor;
use crate::screen::Frame;

//Il server riceve i frames dal caster e li trasmette ai ricevers
//che poi deserializzano i frames, li decomprimono e li fanno vedere ai clients
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
        //socket è un Mutex e solo un thread alla volta può prenderlo,
        //voglio che se un thread si disconnette, la priorità deve passare 
        //al disconnect e non posso piu' mandare messaggi sul canale
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
        
        //Nel caso specifico, la funzione broadcast_thread può continuare a leggere messaggi dal canale sender_rx e a inviarli ai client 
        //attraverso i socket presenti nella mappa sockets_clone.
        //Allo stesso tempo, il server principale può continuare a gestire le connessioni, aggiornare il numero di client.
        //Il thread principale può continuare ad accettare nuove connessioni da client.
        thread::spawn(move || {
            Self::broadcast_thread(sender_rx, sockets_clone, client_count_clone);
        });

        let sockets_clone = Arc::clone(&sockets);
        let client_count_clone = Arc::clone(&client_count);

        //Riassunto di questo secondo thread secondario:
        //Avvia un server TCP che ascolta sulla porta 9041 per connessioni in ingresso.
        /*Ogni volta che un client si connette, il server:
        Ottiene l'indirizzo del client.
        Clona il socket del client.
        Inserisce il socket nella mappa condivisa sockets.
        Incrementa il contatore di client connessi (client_count).
        Se c'è un errore (ad esempio, se un client non riesce a connettersi), stampa un errore.*/
        thread::spawn(move || {
            let listener = match TcpListener::bind("0.0.0.0:9041") {
                Ok(listener) => listener,
                Err(_) => {
                    return;
                }
            };
            println!("Server started on port 9041");

            for stream in listener.incoming() {
                //Ogni volta che un client si connette al server, 
                //viene restituito un oggetto stream, che rappresenta la connessione TCP.
                match stream {
                    Ok(socket) => {
                        let addr = socket.peer_addr().unwrap(); //Quando un client si connette con successo, il server ottiene l'indirizzo del client con socket.peer_addr().unwrap()
                        println!("Client connected: {}", addr);

                        let sockets = Arc::clone(&sockets_clone);
                        let client_count = Arc::clone(&client_count_clone);

                        {
                            let mut sockets_guard = sockets.lock().unwrap();
                            sockets_guard.insert(addr, socket.try_clone().expect("Failed to clone socket"));
                        }
                        client_count.fetch_add(1, Ordering::SeqCst);
                        /*Ordering::SeqCst sta per "Sequentially Consistent" (Consistenza Sequenziale).
                        È l'ordinamento più forte disponibile, garantendo che tutte le operazioni atomiche siano visibili in modo coerente tra tutti i thread
                        come se fossero eseguite in un ordine sequenziale che rispetta l'ordine in cui sono chiamate nel programma.*/
                    }
                    Err(e) => {
                        eprintln!("Connection failed: {}", e);
                    }
                }
            }
        });

        server
    }

    //e' la funzione chiamata sopra e gestita del thread secondario
    //fa un loop infinito in cui se il recevier riceve frame, lo inoltra attraverso un ciclo for a tutti i client presenti in sockets_lock
    //se la ricezione non va a buon fine, quel client con indirizzo addr e' diaconnesso
    //quindi lo inserisco nel vettore 'disconnected_clients' e alla fine rimuovo i clients che si trovano in questo vettore
    fn broadcast_thread(receiver: mpsc::Receiver<Bytes>, sockets: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>, client_count: Arc<AtomicUsize>){
        loop{ //loop infinito
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
            // Converti il frame in DynamicImage (potrebbe dipendere dal tuo tipo di Frame)
            let image = DynamicImage::ImageRgba8(frame.to_image_buffer());  // Questa parte dipende da come rappresenti il frame
    
            // Comprimilo in formato JPG
            let mut buffer = Vec::new();
            if let Err(e) = image.write_to(&mut Cursor::new(&mut buffer), ImageOutputFormat::Jpeg(75)) {
                eprintln!("Failed to compress image: {}", e);
                return None;
            }
    
            // Dimensione del frame (4 byte) con i dati compressi
            let frame_size = (buffer.len() as u32).to_be_bytes();
    
            // Prepara il buffer con dimensione + dati compressi
            let mut full_buffer = Vec::with_capacity(4 + buffer.len());
            full_buffer.extend_from_slice(&frame_size); // Dimensione del frame
            full_buffer.extend_from_slice(&buffer);     // Dati compressi
    
            Some(Bytes::from(full_buffer))
        } else {
            // Se non in streaming, manda solo prefisso di dimensione 0
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