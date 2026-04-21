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

    drop(tx);

    loop {
        tokio::select! {
            frame_opt = rx.recv() => {
                match frame_opt {
                    Some(frame) => {
                        if stream.write_all(&frame).await.is_err() {
                            let _ = child.kill().await;
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
    }

    let status = child.wait().await?;
    let mut final_frame = Vec::with_capacity(5);

    if let Some(code) = status.code() {
        final_frame.push(3);
        final_frame.extend_from_slice(&(code as u32).to_be_bytes());
    } else if let Some(sig) = status.signal() {
        final_frame.push(4);
        final_frame.extend_from_slice(&(sig as u32).to_be_bytes());
    }

    let _ = stream.write_all(&final_frame).await;
    Ok(())
}
