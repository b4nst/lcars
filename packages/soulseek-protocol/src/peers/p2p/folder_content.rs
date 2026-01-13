use crate::{
    async_trait,
    frame::{read_string, write_string, ParseBytes, ToBytes, STR_LENGTH_PREFIX},
    peers::p2p::PeerMessageCode,
    Serialize,
};
use bytes::Buf;
use std::io::Cursor;
use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};

#[derive(Debug, Serialize)]
pub struct FolderContentsRequest {
    pub files: Vec<String>,
}

impl ParseBytes for FolderContentsRequest {
    fn parse(src: &mut Cursor<&[u8]>) -> std::io::Result<Self> {
        let file_nth = src.get_u32_le();
        let mut folder_content_request = FolderContentsRequest { files: vec![] };

        for _ in 0..file_nth {
            folder_content_request.files.push(read_string(src)?);
        }

        Ok(folder_content_request)
    }
}

#[async_trait]
impl ToBytes for FolderContentsRequest {
    async fn write_to_buf(
        &self,
        buffer: &mut BufWriter<impl AsyncWrite + Unpin + Send>,
    ) -> tokio::io::Result<()> {
        // Calculate total length: message code (4) + file count (4) + all file strings
        let mut files_size: u32 = 0;
        for file in &self.files {
            files_size += STR_LENGTH_PREFIX + file.len() as u32;
        }
        let length = 4 + 4 + files_size; // code + count + files

        buffer.write_u32_le(length).await?;
        buffer
            .write_u32_le(PeerMessageCode::FolderContentsRequest as u32)
            .await?;
        buffer.write_u32_le(self.files.len() as u32).await?;

        for file in &self.files {
            write_string(file, buffer).await?;
        }

        Ok(())
    }
}
