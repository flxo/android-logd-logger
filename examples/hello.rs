use log::*;

fn main() {
    let mut config = android_logd_logger::builder()
        .parse_filters("debug")
        .tag("test tag 1")
        .prepend_module(true)
        .init();

    trace!("hello");
    info!("helloHello");
    warn!("hellohello");
    error!("HELLOHELLO");

    config.filter_level(LevelFilter::Error);
    config.tag_target_strip();
    trace!("hello");
    info!("helloHello");

    config.prepend_module(false);
    warn!("prepend module OFF hellohello");

    error!("ERROR prepend module ON hellohello");
    // Use a custom target string that is used as tag
    info!(target: "custom", "hello custom target");
    info!(target: "taaag", "hello custom target");

    // Invoke a log from a submodule
    hello_again::hello();
}

mod hello_again {
    pub fn hello() {
        log::debug!("target set to hello");
    }
}
