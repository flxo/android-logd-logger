#[cfg(any(target_os = "linux", target_os = "android"))]
#[inline]
pub fn id() -> i32 {
    #[allow(clippy::unnecessary_cast)]
    unsafe {
        libc::gettid() as i32
    }
}

#[cfg(target_os = "macos")]
#[inline]
pub fn id() -> i32 {
    #[allow(clippy::unnecessary_cast)]
    unsafe {
        libc::pthread_self() as i32
    }
}

#[cfg(windows)]
#[inline]
pub fn id() -> i32 {
    unsafe { winapi::um::processthreadsapi::GetCurrentThreadId() as i32 }
}

#[cfg(target_os = "redox")]
#[inline]
pub fn id() -> i32 {
    // Each thread has a separate pid on Redox.
    syscall::getpid().unwrap() as i32
}
