use crate::{async_trait, frame::ToBytes, server::MessageCode, Deserialize, Serialize};
use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};

#[derive(Debug, Serialize, Deserialize)]
pub struct SharedFolderAndFiles {
    pub dirs: u32,
    pub files: u32,
}

impl SharedFolderAndFiles {
    pub fn new(dirs: u32, files: u32) -> Self {
        Self { dirs, files }
    }
}

#[async_trait]
impl ToBytes for SharedFolderAndFiles {
    async fn write_to_buf(
        &self,
        buffer: &mut BufWriter<impl AsyncWrite + Unpin + Send>,
    ) -> tokio::io::Result<()> {
        buffer.write_u32_le(8).await?;
        buffer
            .write_u32_le(MessageCode::SharedFoldersAndFiles as u32)
            .await?;
        buffer.write_u32_le(self.dirs).await?;
        buffer.write_u32_le(self.files).await?;
        Ok(())
    }
}
