#![cfg(target_os = "linux")]

use serde::{Deserialize, Serialize};
use std::os::unix::process::ExitStatusExt;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::process::Command;

/// Represents an ExecSync request payload.
#[derive(Serialize, Deserialize, Debug)]
pub struct ExecSyncRequest {
    pub command: Vec<String>,
}

/// Represents an ExecSync response payload.
#[derive(Serialize, Deserialize, Debug)]
pub struct ExecSyncResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Handles an incoming ExecSync connection over a generic asynchronous stream.
///
/// This function reads a JSON-encoded command array, executes it, and returns the stdout.
pub async fn handle_exec_connection<T>(mut stream: T) -> std::io::Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let mut buffer = [0u8; 1024];
    let n = stream.read(&mut buffer).await?;
    if n == 0 {
        return Ok(());
    }

    let request: ExecSyncRequest = serde_json::from_slice(&buffer[..n])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    if request.command.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Empty command",
        ));
    }

    let mut cmd = Command::new(&request.command[0]);
    if request.command.len() > 1 {
        cmd.args(&request.command[1..]);
    }

    let output = cmd.output().await?;

    // For the prototype, we return the raw stdout bytes as requested.
    // The prompt says: "Write the stdout bytes back to the stream."
    stream.write_all(&output.stdout).await?;

    if let Some(code) = output.status.code() {
        stream.write_all(&[3]).await?;
        stream.write_all(&(code as u32).to_be_bytes()).await?;
    } else if let Some(signal) = output.status.signal() {
        stream.write_all(&[4]).await?;
        stream.write_all(&(signal as u32).to_be_bytes()).await?;
    }

    stream.flush().await?;

    Ok(())
}
