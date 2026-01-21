//! TCP connection handling for Soulseek server communication.
//!
//! Manages the persistent TCP connection to the Soulseek server,
//! including frame parsing and message serialization.

use bytes::BytesMut;
use soulseek_protocol::{
    frame::ToBytes,
    server::{request::ServerRequest, response::ServerResponse},
    SlskError,
};
use std::io::Cursor;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::error::{AppError, Result};

/// Buffer size for reading from the socket.
const READ_BUFFER_SIZE: usize = 8192;

/// Soulseek server connection wrapper.
///
/// Handles TCP communication with the Soulseek server, including
/// message framing and async send/receive.
pub struct SoulseekConnection {
    /// Writer half of the TCP connection.
    writer: Arc<Mutex<BufWriter<OwnedWriteHalf>>>,
    /// Channel to signal shutdown to the read loop.
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl SoulseekConnection {
    /// Connect to the Soulseek server and return a connection handle and message receiver.
    ///
    /// The returned receiver will receive all incoming `ServerResponse` messages.
    /// A background task is spawned to read from the socket and parse messages.
    pub async fn connect(host: &str, port: u16) -> Result<(Self, mpsc::Receiver<ServerResponse>)> {
        let addr = format!("{}:{}", host, port);
        tracing::debug!(addr = %addr, "Connecting to Soulseek server");

        let stream = TcpStream::connect(&addr).await.map_err(|e| {
            AppError::ServiceUnavailable(format!("Failed to connect to Soulseek server: {}", e))
        })?;

        let (reader, writer) = stream.into_split();
        let writer = Arc::new(Mutex::new(BufWriter::new(writer)));

        // Create channel for incoming messages
        let (message_tx, message_rx) = mpsc::channel(100);

        // Create shutdown signal
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // Spawn the read loop
        tokio::spawn(Self::read_loop(reader, message_tx, shutdown_rx));

        tracing::info!(addr = %addr, "Connected to Soulseek server");

        Ok((
            Self {
                writer,
                shutdown_tx: Some(shutdown_tx),
            },
            message_rx,
        ))
    }

    /// Send a request to the server.
    pub async fn send(&self, request: ServerRequest) -> Result<()> {
        let mut writer = self.writer.lock().await;
        request
            .write_to_buf(&mut *writer)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to write request: {}", e)))?;
        writer
            .flush()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to flush writer: {}", e)))?;
        Ok(())
    }

    /// Shutdown the connection gracefully.
    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Background task that reads from the socket and parses messages.
    async fn read_loop(
        mut reader: OwnedReadHalf,
        message_tx: mpsc::Sender<ServerResponse>,
        mut shutdown_rx: oneshot::Receiver<()>,
    ) {
        let mut buffer = BytesMut::with_capacity(READ_BUFFER_SIZE);

        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    tracing::debug!("Read loop received shutdown signal");
                    break;
                }
                result = reader.read_buf(&mut buffer) => {
                    match result {
                        Ok(0) => {
                            tracing::info!("Server closed connection");
                            break;
                        }
                        Ok(n) => {
                            tracing::trace!(bytes = n, "Received data from server");

                            // Try to parse messages from buffer
                            while let Some(response) = Self::try_parse_message(&mut buffer) {
                                match response {
                                    Ok(msg) => {
                                        tracing::trace!(?msg, "Parsed server message");
                                        if message_tx.send(msg).await.is_err() {
                                            tracing::debug!("Message receiver dropped, stopping read loop");
                                            return;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, "Failed to parse server message");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Error reading from server");
                            break;
                        }
                    }
                }
            }
        }

        tracing::debug!("Read loop terminated");
    }

    /// Try to parse a complete message from the buffer.
    ///
    /// Returns `None` if there isn't enough data for a complete message.
    fn try_parse_message(
        buffer: &mut BytesMut,
    ) -> Option<std::result::Result<ServerResponse, SlskError>> {
        if buffer.len() < 8 {
            // Need at least 8 bytes for header (4 length + 4 code)
            return None;
        }

        // Peek at the message length without consuming
        let msg_len = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;

        // Check if we have the complete message (length prefix + message body)
        if buffer.len() < 4 + msg_len {
            return None;
        }

        // We have a complete message, parse it
        let msg_data = buffer.split_to(4 + msg_len);
        let mut cursor = Cursor::new(msg_data.as_ref());

        // Check and parse the message
        match ServerResponse::check(&mut cursor) {
            Ok(header) => Some(
                ServerResponse::parse(&mut cursor, &header)
                    .map_err(|e| SlskError::from(e.to_string())),
            ),
            Err(e) => Some(Err(e)),
        }
    }
}

impl Drop for SoulseekConnection {
    fn drop(&mut self) {
        // Signal shutdown if not already done
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_parse_message_incomplete() {
        let mut buffer = BytesMut::from(&[0u8, 0, 0, 0][..]);
        assert!(SoulseekConnection::try_parse_message(&mut buffer).is_none());
    }

    #[test]
    fn test_try_parse_message_partial_header() {
        let mut buffer = BytesMut::from(&[10u8, 0, 0][..]); // Only 3 bytes
        assert!(SoulseekConnection::try_parse_message(&mut buffer).is_none());
    }
}
