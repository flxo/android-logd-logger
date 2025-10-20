//! Android pstore (persistent storage) logging.
//!
//! This module provides functionality for writing logs to Android's pstore filesystem
//! via the `/dev/pmsg0` device. Logs written to pstore survive reboots but not power
//! cycles, making them useful for debugging boot issues and crashes.

use crate::{logging_iterator::NewlineScaledChunkIterator, Buffer, Priority, Record};
use bytes::{BufMut, BytesMut};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    time::UNIX_EPOCH,
};

/// Path to the persistent message character device.
const PMSG0: &str = "/dev/pmsg0";

/// Magic marker value for Android logger protocol.
const ANDROID_LOG_MAGIC_CHAR: u8 = b'l';

/// Maximum size of a log entry payload in bytes.
const ANDROID_LOG_ENTRY_MAX_PAYLOAD: usize = 4068;

/// Sequence number increment when splitting long messages.
const ANDROID_LOG_PMSG_SEQUENCE_INCREMENT: usize = 1000;

/// Maximum sequence number value.
const ANDROID_LOG_PMSG_MAX_SEQUENCE: usize = 256000;

/// Fixed UID value used in pmsg headers.
///
/// This doesn't appear in log output, so we use a dummy value to avoid
/// the overhead of a system call to determine the real UID.
const DUMMY_UID: u16 = 0;

lazy_static::lazy_static! {
    /// Shared file handle to the open pmsg device.
    static ref PMSG_DEV: parking_lot::RwLock<File> = parking_lot::RwLock::new(
        OpenOptions::new().write(true).open(PMSG0).expect("failed to open pmsg device")
    );
}

/// Writes a log message to the pstore via pmsg0.
///
/// Long messages are automatically split into chunks at newline boundaries
/// to stay within the maximum payload size. Each chunk is written as a
/// separate pmsg packet.
pub(crate) fn log(record: &Record) {
    // Iterate over chunks below the maximum payload byte length, scaled to
    // the last newline character. This follows the C implementation:
    // https://cs.android.com/android/platform/superproject/+/master:system/logging/liblog/pmsg_writer.cpp;l=165
    for (idx, msg_part) in NewlineScaledChunkIterator::new(record.message, ANDROID_LOG_ENTRY_MAX_PAYLOAD).enumerate() {
        let sequence_nr = idx * ANDROID_LOG_PMSG_SEQUENCE_INCREMENT;
        if sequence_nr >= ANDROID_LOG_PMSG_MAX_SEQUENCE {
            return;
        }

        log_pmsg_packet(record, msg_part);
    }
}

/// Flushes the pmsg writer.
///
/// Ensures all buffered data is written to the pstore device.
pub(crate) fn flush() -> io::Result<()> {
    let mut pmsg = PMSG_DEV.write();
    pmsg.flush()
}

/// Writes a single pmsg packet for a message chunk.
///
/// Formats the packet according to the Android pmsg protocol with headers
/// for both the pmsg layer and the log layer.
fn log_pmsg_packet(record: &Record, msg_part: &str) {
    const PMSG_HEADER_LEN: u16 = 7;
    const LOG_HEADER_LEN: u16 = 11;
    // The payload is made up by:
    // - 1 byte for the priority
    // - tag bytes + 1 byte zero terminator
    // - message bytes + 1 byte zero terminator
    let payload_len: u16 = (1 + record.tag.bytes().len() + 1 + msg_part.bytes().len() + 1) as u16;

    let packet_len = PMSG_HEADER_LEN + LOG_HEADER_LEN + payload_len;
    let mut buffer = bytes::BytesMut::with_capacity(packet_len as usize);
    let timestamp = record.timestamp.duration_since(UNIX_EPOCH).unwrap();

    write_pmsg_header(&mut buffer, packet_len, DUMMY_UID, record.pid);
    write_log_header(
        &mut buffer,
        record.buffer_id,
        record.thread_id,
        timestamp.as_secs() as u32,
        timestamp.subsec_nanos(),
    );
    write_payload(&mut buffer, record.priority, record.tag, msg_part);

    {
        let mut pmsg = PMSG_DEV.write();
        if let Err(e) = pmsg.write_all(&buffer) {
            eprintln!("Failed to log message part to pmsg: \"{}: {}\": {}", record.tag, msg_part, e);
        }
    }
}

/// Writes the pmsg header to the buffer.
///
/// The pmsg header contains the magic marker, packet length, UID, and PID.
fn write_pmsg_header(buffer: &mut BytesMut, packet_len: u16, uid: u16, pid: u16) {
    // magic logger marker
    // https://cs.android.com/android/platform/superproject/+/master:system/logging/liblog/include/private/android_logger.h;drc=a66c835cf06a1bee5355f8f61bf543d9ab2aa133;bpv=0;bpt=1;l=34
    buffer.put_u8(ANDROID_LOG_MAGIC_CHAR);
    // message length
    buffer.put_u16_le(packet_len);
    buffer.put_u16_le(uid);
    buffer.put_u16_le(pid);
}

/// Writes the log header to the buffer.
///
/// The log header contains the buffer ID, thread ID, and timestamp information.
fn write_log_header(buffer: &mut BytesMut, buffer_id: Buffer, thread_id: u16, timestamp_secs: u32, timestamp_subsec_nanos: u32) {
    buffer.put_u8(buffer_id.into());
    buffer.put_u16_le(thread_id);
    buffer.put_u32_le(timestamp_secs);
    // In the original pmsg writer, the nanoseconds timestamp is hijacked as
    // sequence number:
    // https://cs.android.com/android/platform/superproject/+/master:system/logging/liblog/pmsg_writer.cpp;l=169
    // However this would lead to different timestamps in the `logd` stream and
    // the logs from the `pstore`. We could not find adverse effects from
    // dropping the sequence number and using the real nanoseconds.
    buffer.put_u32_le(timestamp_subsec_nanos);
}

/// Writes the log payload to the buffer.
///
/// The payload contains the priority, tag, and message, each null-terminated.
fn write_payload(buffer: &mut BytesMut, priority: Priority, tag: &str, msg_part: &str) {
    buffer.put_u8(priority as u8);
    // Tag with zero terminator
    buffer.put(tag.as_bytes());
    buffer.put_u8(0);
    // Message part with zero terminator
    buffer.put(msg_part.as_bytes());
    buffer.put_u8(0);
}
