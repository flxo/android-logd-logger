use crate::{logging_iterator::NewlineScaledChunkIterator, thread, Buffer, Priority, Record};
use bytes::{BufMut, BytesMut};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    process,
    time::UNIX_EPOCH,
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
    /// Shared file handle to the open pmsg device.
    static ref PMSG_DEV: parking_lot::RwLock<File> = parking_lot::RwLock::new(
        OpenOptions::new().write(true).open(PMSG0).expect("failed to open pmsg device")
    );
}

/// Send a log message to pmsg0
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

/// Flush the pmsg writer.
pub(crate) fn flush() -> io::Result<()> {
    let mut pmsg = PMSG_DEV.write();
    pmsg.flush()
}

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

    write_pmsg_header(&mut buffer, packet_len, DUMMY_UID, process::id() as u16);
    write_log_header(
        &mut buffer,
        record.buffer_id,
        thread::id() as u16,
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

fn write_pmsg_header(buffer: &mut BytesMut, packet_len: u16, uid: u16, pid: u16) {
    // magic logger marker
    // https://cs.android.com/android/platform/superproject/+/master:system/logging/liblog/include/private/android_logger.h;drc=a66c835cf06a1bee5355f8f61bf543d9ab2aa133;bpv=0;bpt=1;l=34
    buffer.put_u8(ANDROID_LOG_MAGIC_CHAR);
    // message length
    buffer.put_u16_le(packet_len);
    buffer.put_u16_le(uid);
    buffer.put_u16_le(pid);
}

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

fn write_payload(buffer: &mut BytesMut, priority: Priority, tag: &str, msg_part: &str) {
    buffer.put_u8(priority as u8);
    // Tag with zero terminator
    buffer.put(tag.as_bytes());
    buffer.put_u8(0);
    // Message part with zero terminator
    buffer.put(msg_part.as_bytes());
    buffer.put_u8(0);
}
