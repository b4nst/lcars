use crate::{
    async_trait,
    frame::{read_string, write_string, ParseBytes, ToBytes, STR_LENGTH_PREFIX},
    server::MessageCode,
    Deserialize, Serialize,
};
use bytes::Buf;
use std::io::Cursor;
use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};

#[derive(Debug, Serialize, Deserialize)]
pub struct SayInChat {
    pub room: String,
    pub message: String,
}

#[async_trait]
impl ToBytes for SayInChat {
    async fn write_to_buf(
        &self,
        buffer: &mut BufWriter<impl AsyncWrite + Unpin + Send>,
    ) -> tokio::io::Result<()> {
        let len = STR_LENGTH_PREFIX
            + self.message.len() as u32
            + STR_LENGTH_PREFIX
            + self.room.len() as u32
            + 4;
        buffer.write_u32_le(len).await?;
        buffer
            .write_u32_le(MessageCode::SayInChatRoom as u32)
            .await?;
        write_string(&self.room, buffer).await?;
        write_string(&self.message, buffer).await?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub room: String,
    pub username: String,
    pub message: String,
}

impl ParseBytes for ChatMessage {
    fn parse(src: &mut Cursor<&[u8]>) -> std::io::Result<Self> {
        let room = read_string(src)?;
        let username = read_string(src)?;
        let message = read_string(src)?;

        Ok(Self {
            room,
            username,
            message,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrivateMessage {
    id: u32,
    timestamp: u32,
    username: String,
    message: String,
    is_new: bool,
}

impl ParseBytes for PrivateMessage {
    fn parse(src: &mut Cursor<&[u8]>) -> std::io::Result<Self> {
        let id = src.get_u32_le();
        let timestamp = src.get_u32_le();
        let username = read_string(src)?;
        let message = read_string(src)?;
        let is_new = src.get_u8() == 1;

        Ok(Self {
            id,
            timestamp,
            username,
            message,
            is_new,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupMessage {
    pub users: Vec<String>,
    pub message: String,
}

#[async_trait]
impl ToBytes for GroupMessage {
    async fn write_to_buf(
        &self,
        buffer: &mut BufWriter<impl AsyncWrite + Unpin + Send>,
    ) -> tokio::io::Result<()> {
        // Calculate total length: code (4) + user count (4) + all users + message
        let mut users_len: u32 = 0;
        for user in &self.users {
            users_len += STR_LENGTH_PREFIX + user.len() as u32;
        }
        let len = 4 + 4 + users_len + STR_LENGTH_PREFIX + self.message.len() as u32;

        buffer.write_u32_le(len).await?;
        buffer
            .write_u32_le(MessageCode::MessageUsers as u32)
            .await?;
        buffer.write_u32_le(self.users.len() as u32).await?;

        for user in &self.users {
            write_string(user, buffer).await?;
        }

        write_string(&self.message, buffer).await?;
        Ok(())
    }
}
