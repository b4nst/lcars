use crate::{
    async_trait,
    frame::{write_string, ToBytes, STR_LENGTH_PREFIX},
    server::{MessageCode, HEADER_LEN},
    Deserialize, Serialize,
};
use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};

#[derive(Debug, Serialize, Deserialize)]
pub struct PrivilegesGift {
    pub username: String,
    pub days: u32,
}

#[async_trait]
impl ToBytes for PrivilegesGift {
    async fn write_to_buf(
        &self,
        buffer: &mut BufWriter<impl AsyncWrite + Unpin + Send>,
    ) -> tokio::io::Result<()> {
        let len = HEADER_LEN + STR_LENGTH_PREFIX + self.username.len() as u32 + 4;

        buffer.write_u32_le(len).await?;
        buffer
            .write_u32_le(MessageCode::GivePrivileges as u32)
            .await?;
        write_string(&self.username, buffer).await?;
        buffer.write_u32_le(self.days).await?;
        Ok(())
    }
}
