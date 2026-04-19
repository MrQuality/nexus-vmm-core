# ADR-002: Immutable Memory Mapping for ConfigMaps and Secrets

**Author:** Nir Rozen
**Date:** April 18, 2026
**Status:** Accepted

## Context and Problem Statement
A fundamental component of the Nexus-VMM zero-copy data path involves injecting Kubernetes volumes into the guest VM. While standard `virtio-fs` mounts are generally performant, high concurrent reads create severe serialization bottlenecks. This is particularly problematic for heavily accessed, read-only resources like Kubernetes ConfigMaps and Secrets.

## Decision
To bypass the `virtio-fs` scalability trap, the `nexus-memory-mapper` module will treat all Kubernetes ConfigMaps and Secrets as strictly immutable. 

We dictate that these resources must be mapped directly into the guest VM's memory space. This will be achieved by invoking `libc::mmap` with the `PROT_READ` and `MAP_SHARED` flags. By mapping directly to the host's page cache, we establish a read-only zero-copy path that eliminates write-coherency overhead and serialization bottlenecks entirely.

## Consequences
This approach ensures near bare-metal read performance for configuration and secret data. However, it requires absolute enforcement of immutability. The guest VM cannot alter these mounted resources, and any dynamic updates to ConfigMaps or Secrets will necessitate either a re-mapping event coordinated by the memory mapper or a recreation of the VM instance, depending on the lifecycle policies enforced by `nexus-cri`.