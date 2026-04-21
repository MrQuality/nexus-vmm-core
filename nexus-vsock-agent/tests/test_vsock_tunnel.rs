#![cfg(unix)]

use nexus_vsock_agent::{handle_exec_connection, ExecSyncRequest};
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

/// The Contract: Establishing the failing test for AF_VSOCK ExecSync tunneling.
///
/// a. The test must simulate the host nexus-cri binding to an AF_VSOCK socket.
/// b. It must attempt to serialize and send a mock ExecSync payload to the agent's port.
/// c. It must expect a successful response.
#[tokio::test]
async fn test_execsync_vsock_tunneling() {
    // Simulate bi-directional byte stream using tokio::io::duplex
    let (host_side, guest_side) = duplex(1024);

    // Prepare mock ExecSync payload
    // Use 'hostname' as it's available as an executable on both Windows and Linux standard paths
    let request = ExecSyncRequest {
        command: vec!["hostname".to_string()],
    };
    let payload = serde_json::to_vec(&request).expect("Failed to serialize request");

    // Spawn the guest agent handler
    let handler = tokio::spawn(async move {
        handle_exec_connection(guest_side)
            .await
            .expect("Handler failed");
    });

    // Host side: Send request
    let mut host_side = host_side;
    host_side
        .write_all(&payload)
        .await
        .expect("Failed to write to stream");
    
    // Host side: Read response
    let mut response_buffer = Vec::new();
    host_side.read_to_end(&mut response_buffer).await.expect("Failed to read from stream");

    let stdout_len = response_buffer.len().saturating_sub(5);
    let stdout = &response_buffer[..stdout_len];
    let stream_id = response_buffer.get(stdout_len).copied();

    let response_str = String::from_utf8_lossy(stdout);
    
    println!("Response: {:?}", response_str);
    assert!(
        !response_str.is_empty(),
        "Expected non-empty response from 'hostname'"
    );

    if let Some(id) = stream_id {
        assert!(id == 3 || id == 4, "Expected StreamID 3 or 4");
    }

    handler.await.expect("Handler task panicked");
}
