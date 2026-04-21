#![cfg(unix)]

use nexus_vsock_agent::{ExecSyncRequest, handle_exec_connection};
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

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
    host_side
        .read_to_end(&mut response_buffer)
        .await
        .expect("Failed to read from stream");

    let mut cursor = 0;
    let mut stdout_data = Vec::new();
    let mut final_id = None;

    while cursor < response_buffer.len() {
        let stream_id = response_buffer[cursor];
        cursor += 1;

        if stream_id == 3 || stream_id == 4 {
            final_id = Some(stream_id);
            break;
        } else if stream_id == 1 || stream_id == 2 {
            let len_bytes: [u8; 4] = response_buffer[cursor..cursor + 4].try_into().unwrap();
            let len = u32::from_be_bytes(len_bytes) as usize;
            cursor += 4;
            if stream_id == 1 {
                stdout_data.extend_from_slice(&response_buffer[cursor..cursor + len]);
            }
            cursor += len;
        } else {
            panic!("Invalid StreamID: {}", stream_id);
        }
    }

    let response_str = String::from_utf8_lossy(&stdout_data);
    println!("Response: {:?}", response_str);
    assert!(
        !response_str.is_empty(),
        "Expected non-empty response from 'hostname'"
    );
    assert!(final_id.is_some(), "Expected StreamID 3 or 4 terminator");

    handler.await.expect("Handler task panicked");
}
