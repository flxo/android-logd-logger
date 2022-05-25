use std::{
    io::{self, ErrorKind},
    os::unix::net::UnixDatagram,
    path::Path,
    time::{self, SystemTime},
};

use bytes::BufMut;
use parking_lot::RwLockUpgradableReadGuard;

use crate::{thread, Buffer, Event, Priority, LOGGER_ENTRY_MAX_LEN};

/// Logd write socket path
const LOGDW: &str = "/dev/socket/logdw";

const WRITE_TIMEOUT: time::Duration = time::Duration::from_millis(500);

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
        let socket = UnixDatagram::unbound().expect("Failed to create socket");
        socket.connect(path).ok();
        socket
            .set_write_timeout(Some(WRITE_TIMEOUT))
            .expect("Failed to set write timeout");
        let lock = parking_lot::RwLock::new(socket);
        LogdSocket { socket: lock }
    }

    /// Write a log entry to the log daemon. If a first write attempt fails, try to
    /// reconnect to the log daemon and try again.
    pub fn send(&self, buffer: &[u8]) -> io::Result<()> {
        let lock = self.socket.upgradable_read();
        match lock.send(buffer) {
            Ok(_) => (),
            Err(e) if e.kind() == ErrorKind::TimedOut => (), // discard
            Err(_) => {
                // Try to crate an unbounded socket. Expect this to work.
                let socket = UnixDatagram::unbound()?;
                socket
                    .set_write_timeout(Some(WRITE_TIMEOUT))
                    .expect("Failed to set write timeout");

                // Upgrade the read lock and replace the socket if the sent attempt is successful.
                let mut lock = RwLockUpgradableReadGuard::upgrade(lock);
                socket.connect(LOGDW)?;
                socket.send(buffer).ok();
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
    let timestamp = timestamp.duration_since(time::UNIX_EPOCH).expect("Failed to aquire time");
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
    let timestamp = event.timestamp.duration_since(time::UNIX_EPOCH).unwrap();

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
            std::thread::sleep(time::Duration::from_millis(1));
        });
    }

    let start = time::Instant::now();
    while start.elapsed() < time::Duration::from_secs(5) {
        log("test", Buffer::Main, Priority::Info, "test");
    }
}
