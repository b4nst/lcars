//! Incoming peer connection listener for Soulseek file sharing.
//!
//! This module listens for incoming P2P connections from other Soulseek users
//! and handles requests to browse our shares, search our files, and download from us.

use bytes::{Buf, BytesMut};
use soulseek_protocol::{
    frame::ToBytes,
    message_common::ConnectionType,
    peers::{
        connection::{ConnectionMessageCode, ConnectionMessageHeader, PeerConnectionMessage},
        p2p::{
            request::PeerRequest,
            response::PeerResponse,
            transfer::{PlaceInQueueReply, QueueFailed, TransferReply},
            PeerMessageCode, PeerMessageHeader,
        },
    },
    MessageCode, ProtocolHeader, ProtocolMessage,
};
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

use crate::error::{AppError, Result};

use super::shares::ShareIndex;
use super::uploads::UploadQueue;

/// Buffer size for reading from peer connections.
const READ_BUFFER_SIZE: usize = 65536;

/// Handles incoming P2P connections for file sharing.
pub struct PeerListener {
    listener: TcpListener,
    share_index: Arc<RwLock<ShareIndex>>,
    upload_queue: Arc<RwLock<UploadQueue>>,
    our_username: String,
    upload_speed: u32,
}

impl PeerListener {
    /// Bind to the specified port and prepare to accept connections.
    pub async fn bind(
        port: u16,
        share_index: Arc<RwLock<ShareIndex>>,
        upload_queue: Arc<RwLock<UploadQueue>>,
        our_username: String,
        upload_speed: u32,
    ) -> Result<Self> {
        let addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(&addr).await.map_err(|e| {
            AppError::Internal(format!("Failed to bind listener on {}: {}", addr, e))
        })?;

        tracing::info!(port = port, "Peer listener bound");

        Ok(Self {
            listener,
            share_index,
            upload_queue,
            our_username,
            upload_speed,
        })
    }

    /// Run the listener, accepting and handling connections.
    ///
    /// This is a long-running task that should be spawned.
    pub async fn run(self) {
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    tracing::debug!(peer = %addr, "Incoming peer connection");

                    let share_index = Arc::clone(&self.share_index);
                    let upload_queue = Arc::clone(&self.upload_queue);
                    let our_username = self.our_username.clone();
                    let upload_speed = self.upload_speed;

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(
                            stream,
                            addr,
                            share_index,
                            upload_queue,
                            our_username,
                            upload_speed,
                        )
                        .await
                        {
                            tracing::debug!(peer = %addr, error = %e, "Peer connection error");
                        }
                    });
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to accept connection");
                }
            }
        }
    }

    /// Handle a single incoming connection.
    async fn handle_connection(
        mut stream: TcpStream,
        addr: SocketAddr,
        share_index: Arc<RwLock<ShareIndex>>,
        upload_queue: Arc<RwLock<UploadQueue>>,
        our_username: String,
        upload_speed: u32,
    ) -> Result<()> {
        let mut buffer = BytesMut::with_capacity(READ_BUFFER_SIZE);

        // First, read the connection init message
        let init_msg = Self::read_connection_message(&mut stream, &mut buffer).await?;

        let (peer_username, connection_type, _token) = match init_msg {
            PeerConnectionMessage::PeerInit {
                username,
                connection_type,
                token,
            } => {
                tracing::debug!(
                    peer = %addr,
                    username = %username,
                    connection_type = ?connection_type,
                    token = token,
                    "Peer init received"
                );
                (username, connection_type, token)
            }
            PeerConnectionMessage::PierceFirewall(token) => {
                tracing::debug!(
                    peer = %addr,
                    token = token,
                    "PierceFirewall received (not implemented)"
                );
                return Ok(());
            }
        };

        match connection_type {
            ConnectionType::PeerToPeer => {
                Self::handle_p2p_connection(
                    stream,
                    buffer,
                    peer_username,
                    share_index,
                    upload_queue,
                    our_username,
                    upload_speed,
                )
                .await
            }
            ConnectionType::FileTransfer => {
                Self::handle_file_transfer(stream, buffer, peer_username, share_index, upload_queue)
                    .await
            }
            ConnectionType::DistributedNetwork => {
                tracing::trace!(peer = %addr, "Distributed network connection (not implemented)");
                Ok(())
            }
            ConnectionType::HandShake => {
                tracing::trace!(peer = %addr, "Handshake connection (not implemented)");
                Ok(())
            }
        }
    }

    /// Read a connection message (PeerInit or PierceFirewall).
    async fn read_connection_message(
        stream: &mut TcpStream,
        buffer: &mut BytesMut,
    ) -> Result<PeerConnectionMessage> {
        loop {
            // Need at least 5 bytes for header (4 length + 1 code)
            if buffer.len() >= 5 {
                let msg_len =
                    u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;

                // Check if we have the complete message
                if buffer.len() >= 4 + msg_len {
                    let msg_data = buffer.split_to(4 + msg_len);
                    let mut cursor = Cursor::new(msg_data.as_ref());

                    // Skip length
                    cursor.set_position(4);

                    // Read code
                    let code = ConnectionMessageCode::read(&mut cursor);
                    let header = ConnectionMessageHeader::new(msg_len - 1, code);

                    return PeerConnectionMessage::parse(&mut cursor, &header).map_err(|e| {
                        AppError::Internal(format!("Failed to parse connection message: {}", e))
                    });
                }
            }

            // Read more data
            let n = stream
                .read_buf(buffer)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to read from peer: {}", e)))?;

            if n == 0 {
                return Err(AppError::Internal("Peer closed connection".to_string()));
            }
        }
    }

    /// Handle a P2P connection (browse, search, transfer requests).
    async fn handle_p2p_connection(
        mut stream: TcpStream,
        mut buffer: BytesMut,
        peer_username: String,
        share_index: Arc<RwLock<ShareIndex>>,
        upload_queue: Arc<RwLock<UploadQueue>>,
        _our_username: String,
        _upload_speed: u32,
    ) -> Result<()> {
        loop {
            // Try to parse a message from the buffer
            if let Some(msg_result) = Self::try_parse_peer_message(&mut buffer) {
                match msg_result {
                    Ok(msg) => {
                        tracing::debug!(
                            username = %peer_username,
                            message = ?msg,
                            "Received peer message"
                        );

                        match msg {
                            PeerResponse::SharesRequest => {
                                Self::send_shares_reply(&mut stream, &share_index).await?;
                            }
                            PeerResponse::UserInfoRequest => {
                                // We don't implement user info for now
                                tracing::trace!(
                                    username = %peer_username,
                                    "UserInfoRequest not implemented"
                                );
                            }
                            PeerResponse::TransferRequest(transfer_req) => {
                                if transfer_req.is_download_request() {
                                    // Peer wants to download from us
                                    Self::handle_transfer_request(
                                        &mut stream,
                                        &peer_username,
                                        &transfer_req.filename,
                                        transfer_req.ticket,
                                        &share_index,
                                        &upload_queue,
                                    )
                                    .await?;
                                }
                            }
                            PeerResponse::QueueUpload(queue_upload) => {
                                // Peer is queuing a download from us
                                Self::handle_queue_upload(
                                    &mut stream,
                                    &peer_username,
                                    &queue_upload.file_name,
                                    &share_index,
                                    &upload_queue,
                                )
                                .await?;
                            }
                            PeerResponse::PlaceInQueueRequest(place_req) => {
                                Self::handle_place_in_queue_request(
                                    &mut stream,
                                    &peer_username,
                                    &place_req.file_name,
                                    &upload_queue,
                                )
                                .await?;
                            }
                            _ => {
                                tracing::trace!(
                                    username = %peer_username,
                                    message = ?msg,
                                    "Unhandled peer message"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            username = %peer_username,
                            error = %e,
                            "Failed to parse peer message"
                        );
                    }
                }
            }

            // Read more data
            let n = stream
                .read_buf(&mut buffer)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to read from peer: {}", e)))?;

            if n == 0 {
                tracing::debug!(username = %peer_username, "Peer closed connection");
                return Ok(());
            }
        }
    }

    /// Try to parse a peer message from the buffer.
    fn try_parse_peer_message(
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

        // Skip the length prefix
        cursor.set_position(4);

        // Read the message code
        let code = cursor.get_u32_le();
        let peer_code = PeerMessageCode::from(code);

        // Create header
        let header = PeerMessageHeader::new(msg_len - 4, peer_code);

        // Parse the message
        Some(PeerResponse::parse(&mut cursor, &header))
    }

    /// Send our shared directories to the peer.
    async fn send_shares_reply(
        stream: &mut TcpStream,
        share_index: &Arc<RwLock<ShareIndex>>,
    ) -> Result<()> {
        let index = share_index.read().await;
        let dirs = index.to_protocol_directories();
        drop(index);

        let inner = &mut vec![];
        let mut writer = BufWriter::new(inner);

        PeerRequest::SharesReply(dirs)
            .write_to_buf(&mut writer)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to serialize shares reply: {}", e)))?;

        writer
            .flush()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to flush shares reply: {}", e)))?;

        stream
            .write_all(writer.buffer())
            .await
            .map_err(|e| AppError::Internal(format!("Failed to send shares reply: {}", e)))?;

        tracing::debug!("Sent shares reply");
        Ok(())
    }

    /// Handle a transfer request (peer wants to download from us).
    async fn handle_transfer_request(
        stream: &mut TcpStream,
        username: &str,
        filename: &str,
        ticket: u32,
        share_index: &Arc<RwLock<ShareIndex>>,
        upload_queue: &Arc<RwLock<UploadQueue>>,
    ) -> Result<()> {
        let index = share_index.read().await;
        let file = index.get_file(filename);

        let reply = if let Some(shared_file) = file {
            let size = shared_file.size;
            let local_path = shared_file.path.clone();
            drop(index);

            // Check if we have a free slot
            let mut queue = upload_queue.write().await;
            if queue.has_free_slot() {
                queue.enqueue(
                    username.to_string(),
                    filename.to_string(),
                    local_path,
                    size,
                    ticket,
                );
                TransferReply::TransferReplyOk {
                    ticket,
                    file_size: size,
                }
            } else {
                // File exists but no slot available - queue it
                queue.enqueue(
                    username.to_string(),
                    filename.to_string(),
                    local_path,
                    size,
                    ticket,
                );
                let position = queue.get_queue_position(username, filename).unwrap_or(1);
                drop(queue);

                // Send queued response
                TransferReply::TransferRejected {
                    ticket,
                    reason: format!("Queued (position {})", position),
                }
            }
        } else {
            drop(index);
            TransferReply::TransferRejected {
                ticket,
                reason: "File not shared".to_string(),
            }
        };

        // Send reply
        let inner = &mut vec![];
        let mut writer = BufWriter::new(inner);

        PeerRequest::TransferReply(reply)
            .write_to_buf(&mut writer)
            .await
            .map_err(|e| {
                AppError::Internal(format!("Failed to serialize transfer reply: {}", e))
            })?;

        writer
            .flush()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to flush transfer reply: {}", e)))?;

        stream
            .write_all(writer.buffer())
            .await
            .map_err(|e| AppError::Internal(format!("Failed to send transfer reply: {}", e)))?;

        tracing::debug!(username = %username, filename = %filename, ticket = ticket, "Sent transfer reply");
        Ok(())
    }

    /// Handle a queue upload request.
    async fn handle_queue_upload(
        stream: &mut TcpStream,
        username: &str,
        filename: &str,
        share_index: &Arc<RwLock<ShareIndex>>,
        upload_queue: &Arc<RwLock<UploadQueue>>,
    ) -> Result<()> {
        let index = share_index.read().await;

        if let Some(shared_file) = index.get_file(filename) {
            let size = shared_file.size;
            let local_path = shared_file.path.clone();
            drop(index);

            // Add to queue
            let mut queue = upload_queue.write().await;
            queue.enqueue(
                username.to_string(),
                filename.to_string(),
                local_path,
                size,
                0,
            );

            tracing::debug!(username = %username, filename = %filename, "Added to upload queue");
        } else {
            drop(index);

            // Send queue failed
            let inner = &mut vec![];
            let mut writer = BufWriter::new(inner);

            PeerRequest::QueueFailed(QueueFailed::new(
                filename.to_string(),
                "File not shared".to_string(),
            ))
            .write_to_buf(&mut writer)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to serialize queue failed: {}", e)))?;

            writer
                .flush()
                .await
                .map_err(|e| AppError::Internal(format!("Failed to flush queue failed: {}", e)))?;

            stream
                .write_all(writer.buffer())
                .await
                .map_err(|e| AppError::Internal(format!("Failed to send queue failed: {}", e)))?;

            tracing::debug!(username = %username, filename = %filename, "Sent queue failed (file not shared)");
        }

        Ok(())
    }

    /// Handle a place in queue request.
    async fn handle_place_in_queue_request(
        stream: &mut TcpStream,
        username: &str,
        filename: &str,
        upload_queue: &Arc<RwLock<UploadQueue>>,
    ) -> Result<()> {
        let queue = upload_queue.read().await;
        let position = queue.get_queue_position(username, filename).unwrap_or(0);
        drop(queue);

        let inner = &mut vec![];
        let mut writer = BufWriter::new(inner);

        PeerRequest::PlaceInQueueReply(PlaceInQueueReply::new(filename.to_string(), position))
            .write_to_buf(&mut writer)
            .await
            .map_err(|e| {
                AppError::Internal(format!("Failed to serialize place in queue reply: {}", e))
            })?;

        writer.flush().await.map_err(|e| {
            AppError::Internal(format!("Failed to flush place in queue reply: {}", e))
        })?;

        stream.write_all(writer.buffer()).await.map_err(|e| {
            AppError::Internal(format!("Failed to send place in queue reply: {}", e))
        })?;

        tracing::debug!(username = %username, filename = %filename, position = position, "Sent place in queue reply");
        Ok(())
    }

    /// Handle a file transfer connection.
    async fn handle_file_transfer(
        mut stream: TcpStream,
        mut buffer: BytesMut,
        peer_username: String,
        _share_index: Arc<RwLock<ShareIndex>>,
        upload_queue: Arc<RwLock<UploadQueue>>,
    ) -> Result<()> {
        // For file transfers, we need to wait for the peer to tell us which file
        // they want via a ticket. The file transfer protocol is:
        // 1. Peer sends 4-byte ticket
        // 2. We look up the transfer by ticket
        // 3. Stream the file

        // Read ticket
        while buffer.len() < 4 {
            let n = stream
                .read_buf(&mut buffer)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to read ticket: {}", e)))?;

            if n == 0 {
                return Err(AppError::Internal(
                    "Peer closed before sending ticket".to_string(),
                ));
            }
        }

        let ticket = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        buffer.advance(4);

        tracing::debug!(username = %peer_username, ticket = ticket, "File transfer requested");

        // Find the upload by ticket
        let queue = upload_queue.read().await;
        let upload = queue.find_by_ticket(&peer_username, ticket).cloned();
        drop(queue);

        if let Some(upload_state) = upload {
            // Stream the file
            Self::stream_file(&mut stream, &upload_state.local_path, upload_state.size).await?;

            // Mark as completed
            let mut queue = upload_queue.write().await;
            queue.complete(&upload_state.id);

            tracing::info!(
                username = %peer_username,
                filename = %upload_state.filename,
                "File upload completed"
            );
        } else {
            tracing::warn!(
                username = %peer_username,
                ticket = ticket,
                "No upload found for ticket"
            );
        }

        Ok(())
    }

    /// Stream a file to the peer.
    async fn stream_file(stream: &mut TcpStream, path: &std::path::Path, size: u64) -> Result<()> {
        use tokio::fs::File;

        let mut file = File::open(path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to open file for upload: {}", e)))?;

        // Send file size first
        stream
            .write_u64_le(size)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to send file size: {}", e)))?;

        // Stream file contents
        let mut buf = vec![0u8; 65536];
        loop {
            let n = file
                .read(&mut buf)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to read file: {}", e)))?;

            if n == 0 {
                break;
            }

            stream
                .write_all(&buf[..n])
                .await
                .map_err(|e| AppError::Internal(format!("Failed to send file data: {}", e)))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_peer_message_incomplete() {
        let mut buffer = BytesMut::from(&[0u8, 0, 0, 0][..]);
        assert!(PeerListener::try_parse_peer_message(&mut buffer).is_none());
    }

    #[test]
    fn test_parse_peer_message_partial() {
        let mut buffer = BytesMut::from(&[10u8, 0, 0, 0, 1, 2, 3][..]);
        assert!(PeerListener::try_parse_peer_message(&mut buffer).is_none());
    }
}
