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
    // We must signal the end of the request if the handler expects to read everything,
    // but our current handler just does one 'read'.
    // To allow read_to_end on host_side to work, the handler must finish and close the stream.
    
    // Host side: Read response
    let mut response_buffer = Vec::new();
    // Use read to get the first chunk, which should be the hostname
    let mut chunk = [0u8; 1024];
    let n = host_side.read(&mut chunk).await.expect("Failed to read from stream");
    response_buffer.extend_from_slice(&chunk[..n]);

    let response_str = String::from_utf8_lossy(&response_buffer);
    
    println!("Response: {:?}", response_str);
    assert!(
        !response_str.is_empty(),
        "Expected non-empty response from 'hostname'"
    );

    handler.await.expect("Handler task panicked");
}
