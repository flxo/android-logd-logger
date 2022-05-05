use log::*;

fn main() {
    android_logd_logger::builder()
        .parse_filters("debug")
        .tag_target_strip()
        .prepend_module(true)
        .init();

    trace!("hello");
    info!("helloHello");
    warn!("hellohello");
    error!("HELLOHELLO");

    // Use a custom target string that is used as tag
    info!(target: "custom", "hello custom target");

    // Invoke a log from a submodule
    hello_again::hello();
}

mod hello_again {
    pub fn hello() {
        log::debug!("target set to hello");
    }
}
