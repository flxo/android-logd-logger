use crate::{logging_iterator::NewlineScaledChunkIterator, Buffer, Priority, Record};
use bytes::{BufMut, BytesMut};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
};

/// Persistent message charater device
const PMSG0: &str = "/dev/pmsg0";

/// 'Magic' marker value of android logger
const ANDROID_LOG_MAGIC_CHAR: u8 = b'l';
/// Maximum size of log entry payload
const ANDROID_LOG_ENTRY_MAX_PAYLOAD: usize = 4068;
/// Increment of sequence number when breaking messages in the Android logging system
const ANDROID_LOG_PMSG_SEQUENCE_INCREMENT: usize = 1000;
// Maximum sequence number in Android logging system
const ANDROID_LOG_PMSG_MAX_SEQUENCE: usize = 256000;

/// Fixed UID to use. This does not show up in the log output so we save the
/// system call to determine it.
const DUMMY_UID: u16 = 0;

lazy_static::lazy_static! {
    static ref PMSG_DEV: PmsgDev = PmsgDev::connect(PMSG0);
}

/// Persistent message character device abstraction. Can only be written to
/// and not read from.
struct PmsgDev {
    file: parking_lot::RwLock<File>,
}

impl PmsgDev {
    /// Construct a new PmsgDev.
    ///
    /// # Panics
    ///
    /// This function panics if the underlying character device cannot be opened
    /// for writing.
    pub fn connect(path: &str) -> Self {
        let pmsg_dev = OpenOptions::new().write(true).open(path).expect("failed to open pmsg device");

        Self {
            file: parking_lot::RwLock::new(pmsg_dev),
        }
    }

    /// Write a buffer to the pmsg device.
    ///
    /// Structure and length limits for meaningful input has to be handled by
    /// the calling side.
    pub fn write_all(&self, buffer: &[u8]) -> io::Result<()> {
        let mut pmsg = self.file.write();
        pmsg.write_all(buffer)
    }

    /// Flush the backing file handle.
    pub fn flush(&self) -> io::Result<()> {
        let mut pmsg = self.file.write();
        pmsg.flush()
    }
}

/// Send a log message to pmsg0
pub(crate) fn log(record: &Record) {
    let timestamp_secs = record.timestamp.as_secs() as u32;

    for (idx, msg_part) in NewlineScaledChunkIterator::new(record.message, ANDROID_LOG_ENTRY_MAX_PAYLOAD).enumerate() {
        let sequence_nr = idx * ANDROID_LOG_PMSG_SEQUENCE_INCREMENT;
        if sequence_nr >= ANDROID_LOG_PMSG_MAX_SEQUENCE {
            return;
        }

        log_pmsg_packet(
            timestamp_secs,
            record.pid,
            record.thread_id,
            record.buffer_id,
            record.tag,
            record.priority,
            msg_part,
            sequence_nr as u32,
        );
    }
}

/// Flush the pmsg writer.
pub(crate) fn flush() -> io::Result<()> {
    PMSG_DEV.flush()
}

fn log_pmsg_packet(
    timestamp_secs: u32,
    pid: u16,
    thread_id: u16,
    buffer_id: Buffer,
    tag: &str,
    priority: Priority,
    msg_part: &str,
    sequence_nr: u32,
) {
    const PMSG_HEADER_LEN: u16 = 7;
    const LOG_HEADER_LEN: u16 = 11;
    // The payload is made up by:
    // - 1 byte for the priority
    // - tag bytes + 1 byte zero terminator
    // - message bytes + 1 byte zero terminator
    let payload_len: u16 = (1 + tag.bytes().len() + 1 + msg_part.bytes().len() + 1) as u16;

    let packet_len = PMSG_HEADER_LEN + LOG_HEADER_LEN + payload_len;
    let mut buffer = bytes::BytesMut::with_capacity(packet_len as usize);

    write_pmsg_header(&mut buffer, packet_len, DUMMY_UID, pid);
    write_log_header(&mut buffer, buffer_id, thread_id, timestamp_secs, sequence_nr);
    write_payload(&mut buffer, priority, tag, msg_part);

    if let Err(e) = PMSG_DEV.write_all(&buffer) {
        eprintln!("Failed to log message part to pmsg: \"{}: {}\": {}", tag, msg_part, e);
    }
}

fn write_pmsg_header(buffer: &mut BytesMut, packet_len: u16, uid: u16, pid: u16) {
    // magic logger marker
    // https://cs.android.com/android/platform/superproject/+/master:system/logging/liblog/include/private/android_logger.h;drc=a66c835cf06a1bee5355f8f61bf543d9ab2aa133;bpv=0;bpt=1;l=34
    buffer.put_u8(ANDROID_LOG_MAGIC_CHAR);
    // message length
    buffer.put_u16_le(packet_len);
    buffer.put_u16_le(uid);
    buffer.put_u16_le(pid);
}

fn write_log_header(buffer: &mut BytesMut, buffer_id: Buffer, thread_id: u16, timestamp_secs: u32, sequence_nr: u32) {
    buffer.put_u8(buffer_id.into());
    buffer.put_u16_le(thread_id);
    buffer.put_u32_le(timestamp_secs);
    // The nanoseconds timestamp is hijacked as sequence number:
    // https://cs.android.com/android/platform/superproject/+/master:system/logging/liblog/pmsg_writer.cpp;l=169
    buffer.put_u32_le(sequence_nr);
}

fn write_payload(buffer: &mut BytesMut, priority: Priority, tag: &str, msg_part: &str) {
    buffer.put_u8(priority as u8);
    // Tag with zero terminator
    buffer.put(tag.as_bytes());
    buffer.put_u8(0);
    // Message part with zero terminator
    buffer.put(msg_part.as_bytes());
    buffer.put_u8(0);
}
