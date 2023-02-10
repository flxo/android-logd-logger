// Temporary test binary to be deleted

use android_logd_logger::{tmp_log, Buffer};

pub fn main() {
    for buffer_id in [Buffer::Main, Buffer::System, Buffer::Security, Buffer::Events, Buffer::Radio] {
        for priority in 2..=6 {
            tmp_log(
                "SimonsComponent",
                buffer_id,
                priority,
                &format!("A message with buffer {buffer_id:?} and priority {priority}"),
            );
        }
    }
}
