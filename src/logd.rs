use std::{
    io::{self, ErrorKind},
    os::unix::net::UnixDatagram,
    path::Path,
    time::SystemTime,
};

use bytes::BufMut;
use parking_lot::RwLockUpgradableReadGuard;

use crate::{thread, Buffer, Event, Priority, LOGGER_ENTRY_MAX_LEN};

/// Logd write socket path
const LOGDW: &str = "/dev/socket/logdw";

lazy_static::lazy_static! {
    static ref SOCKET: LogdSocket = LogdSocket::connect(Path::new(LOGDW));
}

/// Logd write socket abstraction. Sends never fail and on each send a reconnect
/// attempt is made.
struct LogdSocket {
    socket: parking_lot::RwLock<UnixDatagram>,
}

impl LogdSocket {
    /// Construct a new LogdSocket.
    ///
    /// # Panics
    ///
    /// This function panics when the socket cannot be created.
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

    /// Write a log entry to the log daemon. If a first write attempt fails, try to
    /// reconnect to the log daemon and try again.
    pub fn send(&self, buffer: &[u8]) -> io::Result<()> {
        let lock = self.socket.upgradable_read();
        match lock.send(buffer) {
            Ok(_) => (),
            Err(e) if e.kind() == ErrorKind::WouldBlock => (), // discard
            Err(_) => {
                // Try to crate an unbounded socket. Expect this to work.
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

/// Send a log message to logd
pub(crate) fn log(tag: &str, buffer_id: Buffer, priority: Priority, message: &str) {
    let timestamp = SystemTime::now();

    // Tag and message len with null terminator.
    let tag_len = tag.bytes().len() + 1;
    let message_len = message.bytes().len() + 1;

    let mut buffer = bytes::BytesMut::with_capacity(12 + tag_len + message_len);

    buffer.put_u8(buffer_id.into());
    buffer.put_u16_le(thread::id() as u16);
    let timestamp = timestamp
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Failed to aquire time");
    buffer.put_u32_le(timestamp.as_secs() as u32);
    buffer.put_u32_le(timestamp.subsec_nanos());
    buffer.put_u8(priority as u8);
    buffer.put(tag.as_bytes());
    buffer.put_u8(0);

    buffer.put(message.as_bytes());
    buffer.put_u8(0);

    if let Err(e) = SOCKET.send(&buffer) {
        eprintln!("Failed to send log message \"{}: {}\": {}", tag, message, e);
    }
}

/// Send a log event to logd
pub(crate) fn write_event(log_buffer: Buffer, event: &Event) {
    let mut buffer = bytes::BytesMut::with_capacity(LOGGER_ENTRY_MAX_LEN);
    let timestamp = event.timestamp.duration_since(std::time::UNIX_EPOCH).unwrap();

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
        log("test", Buffer::Main, Priority::Info, "test");
    }
}
