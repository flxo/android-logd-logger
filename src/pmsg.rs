use crate::thread;
use crate::{Buffer, Priority};
use bytes::BufMut;
use bytes::BytesMut;
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::Path,
    time::SystemTime,
};

/// Persistent message charater device
const PMSG0: &str = "/dev/pmsg0";

/// 'Magic' marker value of android logger
const LOGGER_MAGIC: u8 = 'l' as u8;

/// Maximum size of log entry payload
const LOGGER_ENTRY_MAX_PAYLOAD: u16 = 4068;

lazy_static::lazy_static! {
    static ref PMSG_DEV: PmsgDev = PmsgDev::connect(Path::new(PMSG0));
}

struct PmsgDev {
    file: parking_lot::RwLock<File>,
}

impl PmsgDev {
    pub fn connect(path: &Path) -> Self {
        let pmsg_dev = OpenOptions::new().write(true).open(path).expect("failed to open pmsg device");

        Self {
            file: parking_lot::RwLock::new(pmsg_dev),
        }
    }

    pub fn write_all(&self, buffer: &[u8]) -> io::Result<()> {
        let mut pmsg = self.file.write();
        pmsg.write_all(buffer)
    }
}

/// Send a log message to pmsg0
pub(crate) fn log(tag: &str, buffer_id: Buffer, priority: Priority, message: &str) {
    // TODO: The timestamps of logd and pmsg won't match if we create them separately
    let timestamp_as_duration = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("failed to aquire time");
    let timestamp_secs = timestamp_as_duration.as_secs() as u32;

    // TODO:
    // Calculate how many bytes fit in an entry given the overhead
    // Split the message into pieces and write them individually until
    // everything is written out
    // If there is a newline somewhere towards the end of a piece, split there
    log_pmsg_packet(tag, buffer_id, priority, message, timestamp_secs, 0);
}

fn log_pmsg_packet(tag: &str, buffer_id: Buffer, priority: Priority, msg_part: &str, timestamp_secs: u32, sequence_nr: u32) {
    const PMSG_HEADER_LEN: u16 = 7;
    const LOG_HEADER_LEN: u16 = 11;
    // The payload is made up by:
    // - 1 byte for the priority
    // - tag bytes + 1 byte zero terminator
    // - message bytes + 1 byte zero terminator
    let payload_len: u16 = (1 + tag.bytes().len() + 1 + msg_part.bytes().len() + 1) as u16;

    let mut buffer = bytes::BytesMut::new();

    let packet_len = PMSG_HEADER_LEN + LOG_HEADER_LEN + payload_len;
    // TODO: Fetch uid and pid
    let uid = 0;
    let pid = 2;
    let thread_id = thread::id() as u16;

    write_pmsg_header(&mut buffer, packet_len, uid, pid);
    write_log_header(&mut buffer, buffer_id, thread_id, timestamp_secs, sequence_nr);
    write_payload(&mut buffer, priority, tag, msg_part);

    if let Err(e) = PMSG_DEV.write_all(&buffer) {
        eprintln!("Failed to log message part to pmsg: \"{}: {}\": {}", tag, msg_part, e);
    }
}

fn write_pmsg_header(buffer: &mut BytesMut, packet_len: u16, uid: u16, pid: u16) {
    // magic logger marker
    // https://cs.android.com/android/platform/superproject/+/master:system/logging/liblog/include/private/android_logger.h;drc=a66c835cf06a1bee5355f8f61bf543d9ab2aa133;bpv=0;bpt=1;l=34
    buffer.put_u8(LOGGER_MAGIC);
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

/// Temporary function to try out in a binary without logger integration
pub fn tmp_log(tag: &str, buffer_id: Buffer, priority: u8, message: &str) {
    let priority = match priority {
        2 => Priority::Verbose,
        3 => Priority::Debug,
        4 => Priority::Info,
        5 => Priority::Warn,
        6 => Priority::Error,
        _ => Priority::_Unknown,
    };
    log(tag, buffer_id, priority as Priority, message)
}
