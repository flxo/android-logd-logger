/// Demonstrates how to configure the logger *after* initialization.
use log::*;

fn main() {
    let logger = android_logd_logger::builder().filter_level(LevelFilter::Info).init();

    info!("hello?");

    // Use a custom target string that is used as tag.
    logger.tag_target();
    info!(target: "custom", "hello custom target!");

    logger.tag("whooha");
    info!("hello whooha");
}
