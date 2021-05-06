use android_logd_logger::{write_event, write_event_now, Error, Event, EventValue};

fn main() -> Result<(), Error> {
    android_logd_logger::builder().init();

    write_event(&Event {
        timestamp: std::time::SystemTime::now(),
        tag: 1,
        value: "blah".into(),
    })?;

    write_event_now(1, "test")?;

    let value: Vec<EventValue> = vec![1.into(), "one".into(), 123.3.into()].into();
    write_event_now(2, value)?;

    Ok(())
}
