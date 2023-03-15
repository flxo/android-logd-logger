#[cfg(unix)]
#[inline]
pub fn id() -> usize {
    #[allow(clippy::unnecessary_cast)]
    unsafe {
        libc::pthread_self() as usize
    }
}

#[cfg(windows)]
#[inline]
pub fn id() -> usize {
    unsafe { winapi::um::processthreadsapi::GetCurrentThreadId() as usize }
}

#[cfg(target_os = "redox")]
#[inline]
pub fn id() -> usize {
    // Each thread has a separate pid on Redox.
    syscall::getpid().unwrap()
}
