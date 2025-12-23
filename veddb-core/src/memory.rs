//! Shared memory management utilities
//!
//! Provides cross-platform shared memory creation and mapping with support for:
//! - memfd on Linux (preferred)
//! - Named shared memory as fallback
//! - Proper cleanup and error handling

use anyhow::{Context, Result};
use memmap2::MmapMut;
use std::fs::OpenOptions;
#[cfg(target_os = "linux")]
use std::os::unix::io::{AsRawFd, FromRawFd};

#[cfg(target_os = "linux")]
use nix::sys::memfd::{memfd_create, MemFdCreateFlag};
#[cfg(target_os = "linux")]
use nix::unistd::ftruncate;
#[cfg(target_os = "linux")]
use std::ffi::CString;
#[cfg(target_os = "linux")]
use std::os::fd::{AsFd, OwnedFd};

/// Shared memory segment that can be accessed by multiple processes
pub struct SharedMemory {
    pub mmap: MmapMut,
    pub size: usize,
    _cleanup: Option<Box<dyn FnOnce() + Send>>,
}

impl SharedMemory {
    /// Create a new shared memory segment with the given size
    ///
    /// On Linux, uses memfd_create for anonymous shared memory.
    /// Falls back to named shared memory on other platforms.
    pub fn create(name: &str, size: usize) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            Self::create_memfd(name, size).or_else(|_| Self::create_named(name, size))
        }

        #[cfg(not(target_os = "linux"))]
        {
            Self::create_named(name, size)
        }
    }

    /// Create shared memory using memfd (Linux only)
    #[cfg(target_os = "linux")]
    fn create_memfd(name: &str, size: usize) -> Result<Self> {
        // Convert &str to CString for nix API
        let c_name = CString::new(name).context("Invalid name for memfd")?;
        let fd = memfd_create(&c_name, MemFdCreateFlag::MFD_CLOEXEC)
            .context("Failed to create memfd")?;

        // ftruncate now takes a reference to OwnedFd
        ftruncate(fd.as_fd(), size as i64).context("Failed to set memfd size")?;

        // Convert OwnedFd to File with explicit type
        let file: std::fs::File = fd.into();
        let mmap = unsafe { MmapMut::map_mut(&file) }.context("Failed to mmap memfd")?;

        Ok(Self {
            mmap,
            size,
            _cleanup: None,
        })
    }

    /// Create shared memory using named shm
    fn create_named(name: &str, size: usize) -> Result<Self> {
        #[cfg(target_os = "linux")]
        let path = {
            let shm_name = format!("/veddb_{}", name);
            format!("/dev/shm{}", shm_name)
        };

        #[cfg(not(target_os = "linux"))]
        let path = {
            // Use temp directory on non-Linux platforms (e.g., Windows)
            let mut p = std::env::temp_dir();
            p.push(format!("veddb_{}.shm", name));
            p.to_string_lossy().into_owned()
        };

        // Create the shared memory file
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .context("Failed to create named shared memory")?;

        file.set_len(size as u64)
            .context("Failed to set shared memory size")?;

        let mmap = unsafe { MmapMut::map_mut(&file) }.context("Failed to mmap shared memory")?;

        let cleanup_path = path.clone();
        let cleanup = Box::new(move || {
            let _ = std::fs::remove_file(&cleanup_path);
        });

        Ok(Self {
            mmap,
            size,
            _cleanup: Some(cleanup),
        })
    }

    /// Open an existing shared memory segment
    pub fn open(name: &str) -> Result<Self> {
        #[cfg(target_os = "linux")]
        let path = {
            let shm_name = format!("/veddb_{}", name);
            format!("/dev/shm{}", shm_name)
        };

        #[cfg(not(target_os = "linux"))]
        let path = {
            let mut p = std::env::temp_dir();
            p.push(format!("veddb_{}.shm", name));
            p.to_string_lossy().into_owned()
        };

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .context("Failed to open shared memory")?;

        let mmap = unsafe { MmapMut::map_mut(&file) }.context("Failed to mmap shared memory")?;

        let size = mmap.len();

        Ok(Self {
            mmap,
            size,
            _cleanup: None, // Don't cleanup when opening existing
        })
    }

    /// Get a pointer to the start of the shared memory
    pub fn as_ptr(&self) -> *mut u8 {
        self.mmap.as_ptr() as *mut u8
    }

    /// Get the base address as a usize for offset calculations
    pub fn base_addr(&self) -> usize {
        self.as_ptr() as usize
    }

    /// Convert an offset to a pointer
    ///
    /// # Safety
    /// The caller must ensure the offset is valid and within bounds
    pub unsafe fn offset_to_ptr<T>(&self, offset: u64) -> *mut T {
        (self.as_ptr() as usize + offset as usize) as *mut T
    }

    /// Convert a pointer to an offset
    ///
    /// # Safety
    /// The pointer must be within this shared memory segment
    pub unsafe fn ptr_to_offset<T>(&self, ptr: *const T) -> u64 {
        (ptr as usize - self.base_addr()) as u64
    }
}

impl Drop for SharedMemory {
    fn drop(&mut self) {
        if let Some(cleanup) = self._cleanup.take() {
            cleanup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_memory_creation() {
        let shm = SharedMemory::create("test", 4096).unwrap();
        assert_eq!(shm.size, 4096);

        // Write some data
        unsafe {
            let ptr = shm.as_ptr();
            *ptr = 42;
            assert_eq!(*ptr, 42);
        }
    }

    #[test]
    fn test_offset_conversion() {
        let shm = SharedMemory::create("test_offset", 4096).unwrap();

        unsafe {
            let ptr = shm.offset_to_ptr::<u64>(100);
            *ptr = 0xdeadbeef;

            let offset = shm.ptr_to_offset(ptr);
            assert_eq!(offset, 100);

            let ptr2 = shm.offset_to_ptr::<u64>(offset);
            assert_eq!(*ptr2, 0xdeadbeef);
        }
    }
}
