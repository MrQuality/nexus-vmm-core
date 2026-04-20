//! Nexus Memory Mapper
//!
//! This module implements ADR-002: Immutable Memory Mapping for
//! Kubernetes ConfigMaps and Secrets.

use std::io;
use std::ops::Deref;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(unix)]
use std::ptr;

/// An opaque struct representing memory-mapped secret data.
///
/// This struct provides a safe RAII wrapper around `libc::mmap`.
/// It ensures that `libc::munmap` is called when the struct is dropped.
#[derive(Debug)]
pub struct MappedSecret {
    #[allow(dead_code)]
    ptr: *mut libc::c_void,
    #[allow(dead_code)]
    len: usize,
}

impl MappedSecret {
    /// Creates a new `MappedSecret` from a raw pointer and length.
    ///
    /// # SAFETY:
    /// The caller must ensure that `ptr` points to a valid memory mapping
    /// of at least `len` bytes that can be safely unmapped using `libc::munmap`.
    #[cfg(unix)]
    unsafe fn new(ptr: *mut libc::c_void, len: usize) -> Self {
        Self { ptr, len }
    }
}

impl Deref for MappedSecret {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        #[cfg(unix)]
        {
            // SAFETY: The memory is mapped as PROT_READ and is valid for the lifetime of MappedSecret.
            // We trust libc::mmap to have provided a valid pointer of the requested length.
            unsafe { std::slice::from_raw_parts(self.ptr as *const u8, self.len) }
        }
        #[cfg(not(unix))]
        {
            unreachable!("MappedSecret cannot be constructed on non-unix platforms")
        }
    }
}

impl Drop for MappedSecret {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // SAFETY: The pointer and length were validated upon creation of MappedSecret.
            // munmap is safe to call on memory previously mapped with mmap.
            unsafe {
                if libc::munmap(self.ptr, self.len) != 0 {
                    eprintln!(
                        "Warning: failed to munmap at {:p} with length {}: {}",
                        self.ptr,
                        self.len,
                        io::Error::last_os_error()
                    );
                }
            }
        }
    }
}

/// Maps a file into memory as read-only.
///
/// This function opens the file at the given path, retrieves its metadata to determine the size,
/// and maps it into the process's address space using `libc::mmap` with `PROT_READ` and `MAP_SHARED`.
///
/// # Errors
///
/// Returns an error if the file cannot be opened, metadata cannot be retrieved, or `mmap` fails.
#[cfg(unix)]
pub fn map_secret_read_only(path: &Path) -> io::Result<MappedSecret> {
    use std::fs::File;

    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let len = metadata.len() as usize;

    if len == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Cannot mmap an empty file",
        ));
    }

    let fd = file.as_raw_fd();

    // SAFETY:
    // 1. We have a valid file descriptor `fd` from `file` which remains open for the duration of this call.
    // 2. We use `libc::PROT_READ` to ensure the mapping is read-only, adhering to the ADR-002 immutability rule.
    // 3. `libc::MAP_SHARED` is used to map directly to the host page cache, as required.
    // 4. We handle `libc::MAP_FAILED` and check for errors immediately.
    // 5. The resulting pointer and length are wrapped in `MappedSecret`, which ensures `munmap` is called exactly once.
    unsafe {
        let ptr = libc::mmap(
            ptr::null_mut(),
            len,
            libc::PROT_READ,
            libc::MAP_SHARED,
            fd,
            0,
        );

        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }

        Ok(MappedSecret::new(ptr, len))
    }
}

/// Maps a file into memory as read-only.
///
/// Windows-specific stub for ADR-002.
#[cfg(not(unix))]
pub fn map_secret_read_only(_path: &Path) -> io::Result<MappedSecret> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Memory mapping for ADR-002 is only supported on Unix platforms",
    ))
}
