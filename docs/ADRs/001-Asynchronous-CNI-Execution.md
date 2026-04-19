# ADR-001: Asynchronous CNI Execution

**Author:** Nir Rozen
**Date:** April 18, 2026
**Status:** Accepted

## Context and Problem Statement
In the Nexus-VMM architecture, the `nexus-cri` shim serves as the primary orchestrator, implementing the gRPC Container Runtime Interface (CRI) using `tonic` and `tokio`. A critical operational requirement is executing Container Network Interface (CNI) plugins. However, executing these plugins via the standard blocking `std::process::Command` introduces a severe impedance mismatch: it blocks the async gRPC event loop, leading to thread starvation and unacceptable latency spikes across the orchestration plane. 

## Decision
We strictly reject the use of `std::process::Command` within the `nexus-cri` execution path. 

To prevent thread starvation and maintain a fully non-blocking architecture, we mandate the following standard:
1. **External Binaries:** All invocations of external CNI binaries must exclusively use `tokio::process::Command`.
2. **Synchronous C FFI:** Any synchronous C-library FFI calls required during network setup must be offloaded using `tokio::task::spawn_blocking`.

## Consequences
By adhering to these rules, `nexus-cri` will ensure high throughput and low latency for concurrent gRPC requests, preventing orchestration stalls during complex networking setups. All developers must ensure no synchronous blocking operations leak into the main `tokio` runtime threads.