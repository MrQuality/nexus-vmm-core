#![cfg(target_os = "linux")]

use serde::{Deserialize, Serialize};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::Stdio;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Serialize, Deserialize, Debug)]
pub struct ExecSyncRequest {
    pub command: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExecSyncResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub async fn handle_exec_connection<T>(mut stream: T) -> std::io::Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
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
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        });
    }

    let mut child = cmd.spawn()?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(64);

    let tx_out = tx.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        while let Ok(n) = stdout.read(&mut buf).await {
            if n == 0 {
                break;
            }
            let mut frame = Vec::with_capacity(5 + n);
            frame.push(1);
            frame.extend_from_slice(&(n as u32).to_be_bytes());
            frame.extend_from_slice(&buf[..n]);
            if tx_out.send(frame).await.is_err() {
                break;
            }
        }
    });

    let tx_err = tx.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        while let Ok(n) = stderr.read(&mut buf).await {
            if n == 0 {
                break;
            }
            let mut frame = Vec::with_capacity(5 + n);
            frame.push(2);
            frame.extend_from_slice(&(n as u32).to_be_bytes());
            frame.extend_from_slice(&buf[..n]);
            if tx_err.send(frame).await.is_err() {
                break;
            }
        }
    });

    // 1. Drop the main task's clone of the sender so the channel can close
    drop(tx);

    // 2. Loop UNTIL the channel is completely empty (all producers dropped).
    // This guarantees zero data loss before we await the process exit.
    while let Some(chunk) = rx.recv().await {
        if let Err(e) = stream.write_all(&chunk).await {
            eprintln!("Socket disconnected before all logs were drained: {}", e);
            return Err(e); // Dropping here triggers kill_on_drop
        }
    }

    // 3. ONLY after the logs are fully drained to the socket, await the final status.
    let status = child.wait().await?;

    // 4. Send the Exit Code TLV (StreamID 3) or Signal TLV (StreamID 4)
    // Strictly format with Length (4 bytes) to adhere to TLV constraints.
    if let Some(code) = status.code() {
        let mut exit_frame = vec![3u8]; // StreamID = 3
        exit_frame.extend_from_slice(&4u32.to_be_bytes()); // Length = 4 bytes
        exit_frame.extend_from_slice(&(code as u32).to_be_bytes()); // Payload
        let _ = stream.write_all(&exit_frame).await;
    } else if let Some(signal) = status.signal() {
        let mut sig_frame = vec![4u8]; // StreamID = 4
        sig_frame.extend_from_slice(&4u32.to_be_bytes()); // Length = 4 bytes
        sig_frame.extend_from_slice(&(signal as u32).to_be_bytes()); // Payload
        let _ = stream.write_all(&sig_frame).await;
    }

    let _ = stream.flush().await;
    Ok(())
}
