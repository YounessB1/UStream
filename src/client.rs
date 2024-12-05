use tokio::net::TcpStream;
use tokio::io::{self,AsyncReadExt};
use tokio::sync::{mpsc,watch};
use std::net::SocketAddr;
use bincode;
use crate::screen::Frame;
use tokio::time::{timeout, Duration};

#[derive(Clone)]
pub struct DisconnectHandle {
    shutdown_tx: watch::Sender<bool>,
}

impl DisconnectHandle {
    pub async fn disconnect(self) {
        // Signal the background task to stop
        let _ = self.shutdown_tx.send(true);
    }
}

// The function to connect to the server and start receiving frames
pub async fn connect_to_server(
    ip_address: &str,
) -> Result<(mpsc::Receiver<Option<Frame>>, DisconnectHandle), String > {
    let port = 9041;
    let address_port = format!("{}:{}", ip_address, port);

    let addr: SocketAddr = address_port.parse().map_err(|_| {
        format!("Invalid IP address format: {}", ip_address)
    })?;

    // Attempt to connect to the server
    let mut stream = timeout(Duration::from_secs(10), TcpStream::connect(addr))
        .await
        .map_err(|_| format!("Connection to {}:{} timed out", ip_address, port))?
        .map_err(|_| format!("Connection to {}:{} failed", ip_address, port))?;

    println!("Successfully connected to {}", addr);

    // Create an MPSC channel to send frames from the receiver task
    let (frame_tx, frame_rx) = mpsc::channel(10);

    // Create a watch channel for shutdown signaling
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Spawn a task to handle receiving data from the server
    tokio::spawn(async move {
        loop {
            // Check for shutdown signal
            if *shutdown_rx.borrow() {
                break;
            }

            let mut size_buffer = [0u8; 4];
            match stream.read_exact(&mut size_buffer).await {
                Ok(_) => {
                    let frame_size = u32::from_be_bytes(size_buffer) as usize;
                    if frame_size == 0 {
                        continue;
                    }

                    // Step 2: Read the frame data
                    let mut frame_buffer = vec![0u8; frame_size];
                    match stream.read_exact(&mut frame_buffer).await {
                        Ok(_) => {
                            match bincode::deserialize::<Frame>(&frame_buffer) {
                                Ok(frame) => {
                                    // Step 4: Send the frame to the main application via the channel
                                    if frame_tx.send(Some(frame)).await.is_err() {
                                        // If the receiver side is closed, stop the loop
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
                            eprintln!("Failed to read frame data: {}", e);
                            // Handle EOF or other read errors
                            if e.kind() == io::ErrorKind::UnexpectedEof {
                                println!("Connection closed by server.");
                                if let Err(_e) = frame_tx.send(None).await {
                                    eprintln!("Failed to notify receiver about connection closure");
                                }
                            }
                            break;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read frame size: {}", e);
                    // Handle EOF or other read errors
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        println!("Connection closed by server.");
                        if let Err(_e) = frame_tx.send(None).await {
                            eprintln!("Failed to notify receiver about connection closure");
                        }
                    }
                    break;
                }
            }
        }
        if let Ok(stream_std) = stream.into_std() {
            use std::net::Shutdown;
            if let Err(e) = stream_std.shutdown(Shutdown::Both) {
                eprintln!("Error shutting down the connection: {}", e);
            }
            drop(stream_std);
        }
        println!("Receiver task exiting.");
    });

    // Return the frame receiver and disconnect handle to the caller
    let disconnect_handle = DisconnectHandle { shutdown_tx };
    Ok((frame_rx, disconnect_handle))
}