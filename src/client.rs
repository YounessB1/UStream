use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt};
use tokio::sync::{mpsc,watch};
use std::net::SocketAddr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReceiverError {
    #[error("Failed to connect to server: {0}")]
    ConnectionError(std::io::Error),
    #[error("Failed to receive data: {0}")]
    ReceiveError(std::io::Error),
}

// Define the type for the frame data (adjust as needed for your use case)
type Frame = Vec<u8>;

// Type alias for the frame receiver
pub type FrameReceiver = mpsc::Receiver<Frame>;

// The function to connect to the server and start receiving frames
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
) -> Result<(FrameReceiver, DisconnectHandle), ReceiverError> {
    let port = 9041;
    let address_port = format!("{}:{}", ip_address, port);

    let addr: SocketAddr = address:port.parse().map_err(|e| {
        ReceiverError::ConnectionError(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            e,
        ))
    })?;

    let mut stream = TcpStream::connect(addr)
        .await
        .map_err(ReceiverError::ConnectionError)?;

    println!("Successfully connected to {}", addr);

    // Create an MPSC channel to send frames from the receiver task
    let (frame_tx, frame_rx) = mpsc::channel(10);

    // Create a watch channel for shutdown signaling
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Spawn a task to handle receiving data from the server
    tokio::spawn(async move {
        let mut buffer = vec![0; 4096]; // Adjust buffer size as needed

        loop {
            // Check for shutdown signal
            if *shutdown_rx.borrow() {
                println!("Shutdown signal received, stopping receiver.");
                break;
            }

            match stream.read(&mut buffer).await {
                Ok(0) => {
                    // Connection closed
                    println!("Connection closed by server.");
                    break;
                }
                Ok(n) => {
                    let frame = buffer[..n].to_vec(); // Extract the received frame

                    // Send the frame to the main application via the channel
                    if frame_tx.send(frame).await.is_err() {
                        // If the receiver side is closed, stop the loop
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Failed to receive data: {}", e);
                    break;
                }
            }
        }

        println!("Receiver task exiting.");
    });

    // Return the frame receiver and disconnect handle to the caller
    let disconnect_handle = DisconnectHandle { shutdown_tx };
    Ok((frame_rx, disconnect_handle))
}