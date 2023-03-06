use log::*;

fn main() {
    let config = android_logd_logger::builder()
        .parse_filters("debug")
        .tag("test tag 1")
        .prepend_module(true)
        .init();

    trace!("hello");
    info!("helloHello");
    warn!("hellohello");
    error!("HELLOHELLO");

    config.setter().set_level_filter(LevelFilter::Error);
    config.setter().set_tag_to_target_strip();
    trace!("hello");
    info!("helloHello");

    config.setter().set_prepend_module(false);
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
