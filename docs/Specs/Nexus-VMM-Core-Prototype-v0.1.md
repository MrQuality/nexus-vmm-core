# Nexus-VMM Core Prototype v0.1 Specification

**Author:** Nir Rozen
**Date:** April 18, 2026

## Overview
Nexus-VMM replaces the legacy container-wrapped QEMU model with a direct Rust-VMM executing natively on the KVM interface. This prototype specification outlines the three core modules required to establish our zero-copy, hardware-isolated virtualization runtime for Kubernetes.

## Core Modules

### 1. `nexus-cri`
**Purpose:** The central brain and gRPC orchestrator.
**Technology Stack:** Rust, `tonic`, `tokio`.
**Description:** 
Instead of relying on a standard Kubernetes Operator, `nexus-cri` implements the CRI gRPC interface directly. It acts as the shim that intercepts Kubelet calls (like `RunPodSandbox`). It bypasses standard container runtimes (like `runc`) and invokes the Rust-VMM directly. It maps Kubernetes CPU and memory limits directly to KVM memory slots and vCPU affinities, eliminating nested virtualization "Steal Time". As per ADR-001, all process execution for CNI plugins is strictly asynchronous, utilizing `tokio::process::Command` to prevent thread starvation on the gRPC event loop.

### 2. `nexus-memory-mapper`
**Purpose:** The memory bridge for zero-copy data ingestion.
**Technology Stack:** Rust, `libc`, `nix`.
**Description:**
This module is responsible for managing memory allocations and providing high-performance volume mapping into the guest VM. To avoid the scalability bottlenecks of `virtio-fs` under high concurrency, `nexus-memory-mapper` directly injects read-only Kubernetes resources. As dictated by ADR-002, Kubernetes ConfigMaps and Secrets are treated as strictly immutable and mapped directly to the host page cache using `libc::mmap` with `PROT_READ` and `MAP_SHARED`.

### 3. `nexus-vsock-agent`
**Purpose:** The static guest agent for handling execution voids.
**Technology Stack:** Rust (Static compilation), `AF_VSOCK`.
**Description:**
In the absence of a traditional container runtime within the guest VM, standard Kubelet `ExecSync` commands would normally fail. To resolve this 'ExecSync' void, `nexus-vsock-agent` is deployed as a statically compiled micro-agent running inside the guest VM. When the Kubelet issues an `ExecSync` call, the `nexus-cri` shim intercepts, serializes, and tunnels the request directly to this agent over an `AF_VSOCK` connection. The agent then executes the command natively within the guest and tunnels the output back to the host, ensuring seamless Kubernetes compatibility without a container layer.