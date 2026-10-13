use std::io;

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::{session::Session, Server};

impl Server {
    /// Process one JSON document per input line, writing one response line per call.
    /// Notifications do not produce output. Useful for stdio or custom streams.
    pub async fn serve_stream<R, W>(&self, reader: R, mut writer: W) -> io::Result<()>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut session = Session::new(self.router.clone(), tx);
        loop {
            let size = tokio::select! {
                size = read_line_limited(&mut reader, &mut line, self.max_message_size) => size?,
                Some(message) = rx.recv() => {
                    writer.write_all(message.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                    continue;
                }
            };
            if size == 0 {
                return Ok(());
            }
            if let Some(result) = session.handle(&String::from_utf8_lossy(&line)).await {
                writer.write_all(result.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
            line.clear();
        }
    }

    /// Serve newline-delimited JSON-RPC requests on process stdin and stdout.
    pub async fn serve_stdio(&self) -> io::Result<()> {
        self.serve_stream(tokio::io::stdin(), tokio::io::stdout())
            .await
    }
}

async fn read_line_limited<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    line: &mut Vec<u8>,
    limit: usize,
) -> io::Result<usize> {
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(line.len());
        }
        let count = available
            .iter()
            .position(|&byte| byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if line.len().saturating_add(count) > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "JSON-RPC line exceeds size limit",
            ));
        }
        let complete = available[count - 1] == b'\n';
        line.extend_from_slice(&available[..count]);
        reader.consume(count);
        if complete {
            return Ok(line.len());
        }
    }
}
