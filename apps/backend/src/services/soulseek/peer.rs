//! Peer-to-peer connection handling for Soulseek file transfers.
//!
//! This module manages direct connections to Soulseek peers for browsing
//! shared directories and downloading files.

use bytes::{Buf, BytesMut};
use soulseek_protocol::{
    frame::ToBytes,
    message_common::ConnectionType,
    peers::{
        connection::PeerConnectionMessage,
        p2p::{
            folder_content::FolderContentsRequest, request::PeerRequest, response::PeerResponse,
            shared_directories::SharedDirectories, PeerMessageCode, PeerMessageHeader,
        },
    },
    ProtocolHeader, ProtocolMessage,
};
use std::io::Cursor;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::error::{AppError, Result};

/// Buffer size for reading from peer connections.
const READ_BUFFER_SIZE: usize = 65536;

/// Connection timeout for peer connections.
const CONNECT_TIMEOUT_SECS: u64 = 30;

/// A connection to a Soulseek peer.
///
/// Handles P2P protocol communication for browsing files and downloading.
pub struct PeerConnection {
    /// Username of the peer.
    pub username: String,
    /// Writer half of the TCP connection.
    writer: Arc<Mutex<BufWriter<OwnedWriteHalf>>>,
    /// Channel to signal shutdown to the read loop.
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl PeerConnection {
    /// Connect to a peer using the server-provided connection info.
    ///
    /// This establishes a direct TCP connection to the peer.
    pub async fn connect(
        username: &str,
        ip: std::net::Ipv4Addr,
        port: u32,
        our_username: &str,
        token: u32,
    ) -> Result<(Self, mpsc::Receiver<PeerResponse>)> {
        let addr = format!("{}:{}", ip, port);
        tracing::debug!(
            addr = %addr,
            username = %username,
            token = token,
            "Connecting to peer"
        );

        // Connect with timeout
        let stream = tokio::time::timeout(
            std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS),
            TcpStream::connect(&addr),
        )
        .await
        .map_err(|_| {
            AppError::ServiceUnavailable(format!("Connection to peer {} timed out", username))
        })?
        .map_err(|e| {
            AppError::ServiceUnavailable(format!("Failed to connect to peer {}: {}", username, e))
        })?;

        let (reader, writer) = stream.into_split();
        let writer = Arc::new(Mutex::new(BufWriter::new(writer)));

        // Create channel for incoming messages
        let (message_tx, message_rx) = mpsc::channel(100);

        // Create shutdown signal
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // Send PeerInit message to identify ourselves
        {
            let mut w = writer.lock().await;
            let init = PeerConnectionMessage::PeerInit {
                username: our_username.to_string(),
                connection_type: ConnectionType::PeerToPeer,
                token,
            };
            init.write_to_buf(&mut *w)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to send PeerInit: {}", e)))?;
            w.flush()
                .await
                .map_err(|e| AppError::Internal(format!("Failed to flush writer: {}", e)))?;
        }

        // Spawn the read loop
        let username_clone = username.to_string();
        tokio::spawn(Self::read_loop(
            reader,
            message_tx,
            shutdown_rx,
            username_clone,
        ));

        tracing::info!(addr = %addr, username = %username, "Connected to peer");

        Ok((
            Self {
                username: username.to_string(),
                writer,
                shutdown_tx: Some(shutdown_tx),
            },
            message_rx,
        ))
    }

    /// Send a peer request.
    pub async fn send(&self, request: PeerRequest) -> Result<()> {
        let mut writer = self.writer.lock().await;
        request
            .write_to_buf(&mut *writer)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to write peer request: {}", e)))?;
        writer
            .flush()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to flush writer: {}", e)))?;
        Ok(())
    }

    /// Request the peer's shared directories.
    pub async fn request_shares(&self) -> Result<()> {
        self.send(PeerRequest::SharesRequest).await
    }

    /// Request folder contents for specific directories.
    pub async fn request_folder_contents(&self, folders: Vec<String>) -> Result<()> {
        self.send(PeerRequest::FolderContentsRequest(FolderContentsRequest {
            files: folders,
        }))
        .await
    }

    /// Shutdown the connection gracefully.
    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Background task that reads from the peer and parses messages.
    async fn read_loop(
        mut reader: OwnedReadHalf,
        message_tx: mpsc::Sender<PeerResponse>,
        mut shutdown_rx: oneshot::Receiver<()>,
        username: String,
    ) {
        let mut buffer = BytesMut::with_capacity(READ_BUFFER_SIZE);

        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    tracing::debug!(username = %username, "Peer read loop received shutdown signal");
                    break;
                }
                result = reader.read_buf(&mut buffer) => {
                    match result {
                        Ok(0) => {
                            tracing::info!(username = %username, "Peer closed connection");
                            break;
                        }
                        Ok(n) => {
                            tracing::trace!(username = %username, bytes = n, "Received data from peer");

                            // Try to parse messages from buffer
                            while let Some(response) = Self::try_parse_message(&mut buffer) {
                                match response {
                                    Ok(msg) => {
                                        tracing::trace!(username = %username, ?msg, "Parsed peer message");
                                        if message_tx.send(msg).await.is_err() {
                                            tracing::debug!(username = %username, "Message receiver dropped");
                                            return;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            username = %username,
                                            error = %e,
                                            "Failed to parse peer message"
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(username = %username, error = %e, "Error reading from peer");
                            break;
                        }
                    }
                }
            }
        }

        tracing::debug!(username = %username, "Peer read loop terminated");
    }

    /// Try to parse a complete message from the buffer.
    fn try_parse_message(
        buffer: &mut BytesMut,
    ) -> Option<std::result::Result<PeerResponse, std::io::Error>> {
        // Need at least 8 bytes for header (4 length + 4 code)
        if buffer.len() < 8 {
            return None;
        }

        // Peek at the message length without consuming
        let msg_len = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;

        // Check if we have the complete message
        if buffer.len() < 4 + msg_len {
            return None;
        }

        // We have a complete message, parse it
        let msg_data = buffer.split_to(4 + msg_len);
        let mut cursor = Cursor::new(msg_data.as_ref());

        // Skip the length prefix we already read
        cursor.set_position(4);

        // Read the message code
        let code = cursor.get_u32_le();
        let peer_code = PeerMessageCode::from(code);

        // Create header manually
        let header = PeerMessageHeader::new(msg_len - 4, peer_code);

        // Parse the message
        Some(PeerResponse::parse(&mut cursor, &header))
    }
}

impl Drop for PeerConnection {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Result of browsing a peer's shared files.
#[derive(Debug, Clone)]
pub struct BrowseResult {
    /// Username of the peer.
    pub username: String,
    /// Shared directories and files.
    pub directories: SharedDirectories,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_parse_message_incomplete() {
        let mut buffer = BytesMut::from(&[0u8, 0, 0, 0][..]);
        assert!(PeerConnection::try_parse_message(&mut buffer).is_none());
    }

    #[test]
    fn test_try_parse_message_partial() {
        let mut buffer = BytesMut::from(&[10u8, 0, 0, 0, 1, 2, 3][..]); // Says 10 bytes but only 3 present
        assert!(PeerConnection::try_parse_message(&mut buffer).is_none());
    }
}
