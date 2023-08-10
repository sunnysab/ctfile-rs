use crate::downloader::DownloadTask;
use crate::CtFile;

use anyhow::Result;
use bincode::{Decode, Encode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Encode, Decode)]
pub struct TaskStatus {
    pub name: String,

    pub received: usize,
    pub total: usize,

    pub is_finished: bool,
    pub is_failed: bool,
    pub fail_message: Option<String>,
}

impl From<&DownloadTask> for TaskStatus {
    fn from(task: &DownloadTask) -> Self {
        TaskStatus {
            name: task.name.clone(),
            received: task.progress.received(),
            total: task.progress.total(),
            is_finished: task.progress.is_finished(),
            is_failed: task.progress.is_failed(),
            fail_message: task.progress.get_err_message(),
        }
    }
}

#[derive(Encode, Decode)]
pub enum Command {
    List,
    Add(CtFile),
}

#[derive(Encode, Decode)]
pub enum CommandResult {
    List(Vec<TaskStatus>),
    Add,
}

pub struct BinStream {
    stream: TcpStream,

    tx_buffer: Vec<u8>,
    rx_buffer: Vec<u8>,
}

impl BinStream {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            tx_buffer: vec![0u8; 1024],
            rx_buffer: vec![0u8; 1024],
        }
    }

    pub async fn send<T: Encode>(&mut self, payload: T) -> Result<()> {
        let config = bincode::config::standard();
        let len = bincode::encode_into_slice(payload, &mut self.tx_buffer, config)?;

        self.stream.write_u16(len as u16).await?;
        self.stream.write_all(&self.tx_buffer[..len]).await?;
        Ok(())
    }

    pub async fn recv<T: Decode>(&mut self) -> Result<T> {
        let len = self.stream.read_u16().await? as usize;
        self.stream.read_exact(&mut self.rx_buffer[..len]).await?;
        let config = bincode::config::standard();
        let (result, _) = bincode::decode_from_slice(&self.rx_buffer[..len], config)?;
        Ok(result)
    }
}
