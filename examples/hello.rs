use log::*;
use std::{thread, time};

fn main() {
    logd_logger::builder().parse_filters("debug").tag("hello").init();

    let mut threads = Vec::new();
    for i in 0..100 {
        threads.push(std::thread::spawn(move || {
            trace!("hello");
            thread::sleep(time::Duration::from_millis(i));
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
