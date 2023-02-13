// Temporary test binary to be deleted

use android_logd_logger::{tmp_log, Buffer};

pub fn main() {
    for buffer_id in [
        Buffer::Main,
        Buffer::Radio,
        Buffer::Events,
        Buffer::System,
        Buffer::Crash,
        Buffer::Stats,
        Buffer::Security,
        Buffer::Custom(8),
    ]
    .iter()
    {
        for priority in 2..=6 {
            tmp_log(
                "SimonsComponent",
                *buffer_id,
                priority,
                &format!("A log message with buffer {buffer_id:?} and priority {priority}"),
            );
        }
    }

    // Testing breaking up of strings
    let test_str = "This will be a long string.\n\
                              Break it at the last newline below 50 bytes.\n\
                              This may split words into two.\n\
                              Some chunks are also above the maximum length \
                              without a newline and will be split at the \
                              charater boundary below the maximum length.";

    tmp_log("SimonsMsgInSequences", Buffer::Main, 2, test_str);
}
