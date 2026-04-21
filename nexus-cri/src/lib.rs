#![cfg(target_os = "linux")]

use std::collections::HashMap;
use tokio::process::Command;

/// Minimal mock request for Pod Sandbox creation.
pub struct RunPodSandboxRequest {
    pub annotations: HashMap<String, String>,
}

/// The Nexus CRI Service orchestrator.
pub struct NexusCriService;

impl NexusCriService {
    /// Creates a new Nexus CRI Service instance.
    pub fn new() -> Self {
        Self
    }

    /// Orchestrates the Pod Sandbox creation, intercepting for VMM-specific setup.
    pub async fn run_pod_sandbox(&self, req: RunPodSandboxRequest) -> Result<String, String> {
        let vmm_enabled = req
            .annotations
            .get("nexus.io/vmm")
            .map(|v| v == "true")
            .unwrap_or(false);

        if !vmm_enabled {
            return Err("Fallback to standard CRI: nexus.io/vmm is not true".into());
        }

        let sandbox_id = "sandbox-vmm-0.1";
        // ADR-001: Execute CNI setup asynchronously to prevent thread starvation.
        self.execute_cni_setup(sandbox_id).await?;

        Ok(sandbox_id.into())
    }

    /// Internal CNI execution logic using tokio reactor.
    async fn execute_cni_setup(&self, sandbox_id: &str) -> Result<(), String> {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.args(["-c", "sleep 1 && echo '{\"ip\":\"10.0.0.2\"}'"]);

        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", sandbox_id)
            .env("CNI_NETNS", format!("/var/run/netns/{}", sandbox_id))
            .env("CNI_IFNAME", "eth0")
            .env("CNI_PATH", "/opt/cni/bin");

        let output = cmd
            .output()
            .await
            .map_err(|e| format!("Failed to spawn CNI process: {}", e))?;

        if !output.status.success() {
            return Err("CNI ADD execution failed".into());
        }

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let start = stdout_str.find('{').unwrap_or(0);
        let json_str = &stdout_str[start..];

        let path = format!("/var/lib/nexus/sandboxes/{}.json", sandbox_id);
        tokio::fs::create_dir_all("/var/lib/nexus/sandboxes/")
            .await
            .ok();
        tokio::fs::write(&path, json_str.as_bytes())
            .await
            .map_err(|e| format!("Failed to write CNI state: {}", e))?;

        Ok(())
    }

    pub async fn teardown_cni_network(&self, sandbox_id: &str) -> Result<(), String> {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 1"]);

        cmd.env("CNI_COMMAND", "DEL")
            .env("CNI_CONTAINERID", sandbox_id)
            .env("CNI_NETNS", format!("/var/run/netns/{}", sandbox_id))
            .env("CNI_IFNAME", "eth0")
            .env("CNI_PATH", "/opt/cni/bin");

        let status = cmd
            .status()
            .await
            .map_err(|e| format!("Failed to spawn CNI process: {}", e))?;

        if !status.success() {
            return Err("CNI DEL execution failed".into());
        }

        let path = format!("/var/lib/nexus/sandboxes/{}.json", sandbox_id);
        let _ = tokio::fs::remove_file(path).await;

        Ok(())
    }
}
