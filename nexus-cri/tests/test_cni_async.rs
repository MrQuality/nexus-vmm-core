use nexus_cri::{NexusCriService, RunPodSandboxRequest};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_cni_execution_is_non_blocking() {
    let service = NexusCriService::new();
    let mut annotations = HashMap::new();
    annotations.insert("nexus.io/vmm".into(), "true".into());
    let req = RunPodSandboxRequest { annotations };

    // ADR-001 Verification:
    // We spawn a background task that increments a counter while we await CNI setup.
    // If the CNI setup were blocking (std::process), this counter would not increment
    // because the single-threaded (or even multi-threaded) runtime thread would be pinned.

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    let background_task = tokio::spawn(async move {
        for _ in 0..100 {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // Execute the sandbox setup which takes ~1 second via 'timeout' or 'sleep'
    let result = service.run_pod_sandbox(req).await;

    assert!(
        result.is_ok(),
        "Sandbox creation failed: {:?}",
        result.err()
    );

    let final_count = counter.load(Ordering::SeqCst);

    // If non-blocking, the counter should be significantly > 0.
    // Given 1s of CNI execution and 10ms ticks, we expect ~100.
    assert!(
        final_count > 0,
        "ADR-001 Failure: Background task was starved during CNI execution. Count: {}",
        final_count
    );

    background_task.abort();
}

#[tokio::test]
async fn test_fallback_when_vmm_disabled() {
    let service = NexusCriService::new();
    let mut annotations = HashMap::new();
    annotations.insert("nexus.io/vmm".into(), "false".into());
    let req = RunPodSandboxRequest { annotations };

    let result = service.run_pod_sandbox(req).await;
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        "Fallback to standard CRI: nexus.io/vmm is not true"
    );
}
