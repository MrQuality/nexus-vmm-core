# Nexus-VMM Core

**Notice: This is a research project.**

Nexus-VMM is a high-performance, memory-safe, and native Kubernetes virtualization runtime. Unlike legacy systems that wrap QEMU inside a container, Nexus-VMM acts as a custom Container Runtime Interface (CRI) shim that executes Virtual Machines directly on the host's KVM interface using a stripped-down Rust-VMM. 

## Key Architecture

This repository contains the core components of the Nexus-VMM prototype:
- **`nexus-cri`**: The central brain and gRPC orchestrator that implements the Kubernetes CRI directly.
- **`nexus-memory-mapper`**: The memory bridge that maps Kubernetes ConfigMaps and Secrets directly to the host page cache for zero-copy data ingestion.
- **`nexus-vsock-agent`**: A static guest micro-agent that tunnels Kubelet `ExecSync` calls via `AF_VSOCK`.

## Documentation

Foundational architectural decisions and prototype specifications can be found in the `docs/` directory:
- [Specs: Nexus-VMM Core Prototype v0.1](docs/Specs/Nexus-VMM-Core-Prototype-v0.1.md)
- [ADR-001: Asynchronous CNI Execution](docs/ADRs/001-Asynchronous-CNI-Execution.md)
- [ADR-002: Immutable Memory Mapping for ConfigMaps and Secrets](docs/ADRs/002-Immutable-Memory-Mapping.md)