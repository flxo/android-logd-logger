//! Android logd socket communication.
//!
//! This module handles direct communication with Android's `logd` daemon via
//! Unix domain sockets. It provides low-level functions for writing log messages
//! and events to the various log buffers.

use std::{
    io::{self, ErrorKind},
    os::unix::net::UnixDatagram,
    path::Path,
    time::UNIX_EPOCH,
};

use bytes::BufMut;
use parking_lot::RwLockUpgradableReadGuard;

use crate::{thread, Buffer, Event, Record, LOGGER_ENTRY_MAX_LEN};

/// Path to the logd write socket.
const LOGDW: &str = "/dev/socket/logdw";

lazy_static::lazy_static! {
    static ref SOCKET: LogdSocket = LogdSocket::connect(Path::new(LOGDW));
}

/// Logd write socket abstraction.
///
/// This socket wrapper handles automatic reconnection on failures and uses
/// non-blocking I/O to avoid blocking the application if logd is under heavy load.
/// Failed writes are silently discarded rather than blocking or returning errors.
struct LogdSocket {
    socket: parking_lot::RwLock<UnixDatagram>,
}

impl LogdSocket {
    /// Constructs a new LogdSocket connected to the specified path.
    ///
    /// The socket is created as non-blocking to prevent the application from
    /// hanging if logd is slow or unavailable.
    ///
    /// # Panics
    ///
    /// Panics if the socket cannot be created.
    pub fn connect(path: &Path) -> LogdSocket {
        let socket = UnixDatagram::unbound().expect("failed to create socket");

        // Ignore connect failures because this will be retried.
        socket.connect(path).ok();

        // The logd socket is a datagram socket. If a write fails the logd might be
        // under heavy load and is unable to process this write.
        socket
            .set_nonblocking(true)
            .expect("failed to set the logd socket to non blocking");

        let lock = parking_lot::RwLock::new(socket);
        LogdSocket { socket: lock }
    }

    /// Writes data to the log daemon socket.
    ///
    /// If the first write attempt fails (except for `WouldBlock`), this method
    /// attempts to reconnect to the log daemon and retry the write.
    ///
    /// # Errors
    ///
    /// Returns an error if reconnection or the retry write fails. `WouldBlock`
    /// errors are silently ignored (the log message is dropped).
    pub fn send(&self, buffer: &[u8]) -> io::Result<()> {
        let lock = self.socket.upgradable_read();
        match lock.send(buffer) {
            Ok(_) => (),
            Err(e) if e.kind() == ErrorKind::WouldBlock => (), // discard
            Err(_) => {
                // Try to create an unbounded socket. Expect this to work.
                let socket = UnixDatagram::unbound()?;

                // Upgrade the read lock and replace the socket if the sent attempt is successful.
                let mut lock = RwLockUpgradableReadGuard::upgrade(lock);
                socket.connect(LOGDW)?;
                socket.set_nonblocking(true)?;

                socket.send(buffer)?;

                // Assign the new socket to the lock. In the worst case one or more threads
                // are opening sockets to logd which are immediately closed.
                *lock = socket;
            }
        }
        Ok(())
    }
}

/// Sends a log message to the logd daemon.
///
/// Formats the log record according to the logd protocol and writes it to
/// the logd socket. Failed writes are logged to stderr but do not propagate errors.
pub(crate) fn log(record: &Record) {
    // Tag and message len with null terminator.
    let tag_len = record.tag.len() + 1;
    let message_len = record.message.len() + 1;
    let mut buffer = bytes::BytesMut::with_capacity(12 + tag_len + message_len);
    let timestamp = record.timestamp.duration_since(UNIX_EPOCH).unwrap();

    buffer.put_u8(record.buffer_id.into());
    buffer.put_u16_le(thread::id() as u16);
    buffer.put_u32_le(timestamp.as_secs() as u32);
    buffer.put_u32_le(timestamp.subsec_nanos());
    buffer.put_u8(record.priority as u8);
    buffer.put(record.tag.as_bytes());
    buffer.put_u8(0);

    buffer.put(record.message.as_bytes());
    buffer.put_u8(0);

    if let Err(e) = SOCKET.send(&buffer) {
        eprintln!("Failed to send log message \"{}: {}\": {}", record.tag, record.message, e);
    }
}

/// Sends a binary event to the logd daemon.
///
/// Formats the event according to the logd event protocol and writes it to
/// the specified log buffer. Failed writes are logged to stderr but do not
/// propagate errors.
pub(crate) fn write_event(log_buffer: Buffer, event: &Event) {
    let mut buffer = bytes::BytesMut::with_capacity(LOGGER_ENTRY_MAX_LEN);
    let timestamp = event.timestamp.duration_since(UNIX_EPOCH).unwrap();

    buffer.put_u8(log_buffer.into());
    buffer.put_u16_le(thread::id() as u16);
    buffer.put_u32_le(timestamp.as_secs() as u32);
    buffer.put_u32_le(timestamp.subsec_nanos());
    buffer.put_u32_le(event.tag);
    buffer.put(event.value.as_bytes());
    if let Err(e) = SOCKET.send(&buffer) {
        eprintln!("Failed to write event {:?}: {}", event, e);
    }
}

#[test]
fn smoke() {
    use crate::Priority;
    use std::time::SystemTime;

    let tempdir = tempfile::tempdir().unwrap();
    let socket = tempdir.path().join("socket");

    {
        let socket = socket.to_owned();
        std::thread::spawn(move || loop {
            std::fs::remove_file(&socket).ok();
            let _socket = std::os::unix::net::UnixDatagram::bind(&socket).expect("Failed to bind");
            std::thread::sleep(std::time::Duration::from_millis(1));
        });
    }

    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(5) {
        let timestamp = SystemTime::now();
        let record = Record {
            timestamp,
            pid: std::process::id() as u16,
            thread_id: thread::id() as u16,
            buffer_id: Buffer::Main,
            tag: "test",
            priority: Priority::Info,
            message: "test",
        };
        log(&record);
    }
}
