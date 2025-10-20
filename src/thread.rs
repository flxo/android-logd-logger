//! Thread ID retrieval for different platforms.
//!
//! This module provides a cross-platform way to get the current thread ID,
//! which is used in log entries to identify which thread generated a log message.

/// Returns the current thread ID.
///
/// The implementation varies by platform:
/// - Linux/Android: Uses `gettid()` syscall
/// - macOS: Uses `pthread_self()`
/// - Windows: Uses `GetCurrentThreadId()`
/// - Redox: Uses process ID (each thread has its own PID)
#[cfg(any(target_os = "linux", target_os = "android"))]
#[inline]
pub fn id() -> i32 {
    #[allow(clippy::unnecessary_cast)]
    unsafe {
        libc::gettid() as i32
    }
}

/// Returns the current thread ID on macOS.
#[cfg(target_os = "macos")]
#[inline]
pub fn id() -> i32 {
    #[allow(clippy::unnecessary_cast)]
    unsafe {
        libc::pthread_self() as i32
    }
}

/// Returns the current thread ID on Windows.
#[cfg(windows)]
#[inline]
pub fn id() -> i32 {
    unsafe { winapi::um::processthreadsapi::GetCurrentThreadId() as i32 }
}

/// Returns the current thread ID on Redox OS.
#[cfg(target_os = "redox")]
#[inline]
pub fn id() -> i32 {
    // Each thread has a separate pid on Redox.
    syscall::getpid().unwrap() as i32
}
