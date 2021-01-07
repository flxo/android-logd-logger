use log::*;
use std::thread;

fn main() {
    logd_logger::builder()
        .parse_filters("debug")
        .tag("hello")
        .prepend_module(true)
        .init();

    let mut threads = Vec::new();
    for _ in 0..10 {
        threads.push(thread::spawn(move || {
            trace!("hello");
            debug!("Hello");
            info!("helloHello");
            warn!("hellohello");
            error!("HELLOHELLO");
        }));
    }

    threads.drain(..).for_each(|t| {
        t.join().expect("Failed to join");
    });
}
