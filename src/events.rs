use bytes::{BufMut, Bytes, BytesMut};
use std::{iter::FromIterator, time::SystemTime};

use crate::{Buffer, Error, LOGGER_ENTRY_MAX_LEN};

/// Event tag
pub type EventTag = u32;

/// Event data
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Timestamp
    pub timestamp: SystemTime,
    /// Tag
    pub tag: EventTag,
    /// Value
    pub value: EventValue,
}

/// Event's value
#[derive(Debug, PartialEq, Clone)]
pub enum EventValue {
    /// Void value
    Void,
    /// Int value
    Int(i32),
    /// Long value
    Long(i64),
    /// Float value
    Float(f32),
    /// String value
    String(String),
    /// List of values
    List(Vec<EventValue>),
}

impl EventValue {
    /// Serialied size
    pub fn serialized_size(&self) -> usize {
        match self {
            &EventValue::Void => 0,
            EventValue::Int(_) | EventValue::Float(_) => 1 + 4,
            EventValue::Long(_) => 1 + 8,
            EventValue::String(s) => 1 + 4 + s.as_bytes().len(),
            EventValue::List(l) => 1 + 1 + l.iter().map(EventValue::serialized_size).sum::<usize>(),
        }
    }

    /// Serialize the event value into bytes
    pub fn as_bytes(&self) -> Bytes {
        const EVENT_TYPE_INT: u8 = 0;
        const EVENT_TYPE_LONG: u8 = 1;
        const EVENT_TYPE_STRING: u8 = 2;
        const EVENT_TYPE_LIST: u8 = 3;
        const EVENT_TYPE_FLOAT: u8 = 4;

        let mut buffer = BytesMut::with_capacity(self.serialized_size());
        match self {
            EventValue::Void => (),
            EventValue::Int(num) => {
                buffer.put_u8(EVENT_TYPE_INT);
                buffer.put_i32_le(*num);
            }
            EventValue::Long(num) => {
                buffer.put_u8(EVENT_TYPE_LONG);
                buffer.put_i64_le(*num);
            }
            EventValue::Float(num) => {
                buffer.put_u8(EVENT_TYPE_FLOAT);
                buffer.put_f32_le(*num);
            }
            EventValue::String(string) => {
                buffer.put_u8(EVENT_TYPE_STRING);
                buffer.put_u32_le(string.len() as u32);
                buffer.put(string.as_bytes());
            }
            EventValue::List(values) => {
                buffer.put_u8(EVENT_TYPE_LIST);
                buffer.put_u8(values.len() as u8);
                values.iter().for_each(|value| buffer.put(value.as_bytes()));
            }
        };
        buffer.freeze()
    }
}

impl From<()> for EventValue {
    fn from(_: ()) -> Self {
        EventValue::Void
    }
}

impl From<i32> for EventValue {
    fn from(v: i32) -> Self {
        EventValue::Int(v)
    }
}

impl From<i64> for EventValue {
    fn from(v: i64) -> Self {
        EventValue::Long(v)
    }
}

impl From<f32> for EventValue {
    fn from(v: f32) -> Self {
        EventValue::Float(v)
    }
}

impl From<&str> for EventValue {
    fn from(v: &str) -> Self {
        EventValue::String(v.to_string())
    }
}

impl<T> FromIterator<T> for EventValue
where
    T: Into<EventValue>,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        EventValue::List(iter.into_iter().map(Into::into).collect())
    }
}

impl<T> From<Vec<T>> for EventValue
where
    T: Into<EventValue>,
{
    fn from(mut v: Vec<T>) -> Self {
        EventValue::List(v.drain(..).map(|e| e.into()).collect())
    }
}

/// Write an event with the timestamp now to `Buffer::Events`
/// ```
/// use android_logd_logger::{write_event, write_event_now, Error, Event, EventValue};
/// android_logd_logger::builder().init();
///
/// write_event_now(1, "test").unwrap();
///
/// let value: Vec<EventValue> = vec![1.into(), "one".into(), 123.3.into()].into();
/// write_event_now(2, value).unwrap();
/// ```
pub fn write_event_now<T: Into<EventValue>>(tag: EventTag, value: T) -> Result<(), Error> {
    write_event(&Event {
        timestamp: SystemTime::now(),
        tag,
        value: value.into(),
    })
}

/// Write an event with the timestamp now to buffer
/// ```
/// use android_logd_logger::{write_event_buffer_now, Buffer, Error, Event, EventValue};
/// android_logd_logger::builder().init();
///
/// write_event_buffer_now(Buffer::Stats, 1, "test").unwrap();
///
/// let value: Vec<EventValue> = vec![1.into(), "one".into(), 123.3.into()].into();
/// write_event_buffer_now(Buffer::Stats, 2, value).unwrap();
/// ```
pub fn write_event_buffer_now<T: Into<EventValue>>(log_buffer: Buffer, tag: EventTag, value: T) -> Result<(), Error> {
    write_event_buffer(
        log_buffer,
        &Event {
            timestamp: SystemTime::now(),
            tag,
            value: value.into(),
        },
    )
}

/// Write an event to `Buffer::Events`
/// ```
/// use android_logd_logger::{write_event, Error, Event, EventValue};
/// android_logd_logger::builder().init();
///
/// write_event(&Event {
///     timestamp: std::time::SystemTime::now(),
///     tag: 1,
///     value: "blah".into(),
/// }).unwrap();
/// ```
pub fn write_event(event: &Event) -> Result<(), Error> {
    write_event_buffer(Buffer::Events, event)
}

/// Write an event to an explicit buffer
/// ```
/// use android_logd_logger::{write_event_buffer, Buffer, Error, Event, EventValue};
/// android_logd_logger::builder().init();
///
/// write_event_buffer(Buffer::Stats, &Event {
///     timestamp: std::time::SystemTime::now(),
///     tag: 1,
///     value: "blah".into(),
/// }).unwrap();
/// ```
pub fn write_event_buffer(log_buffer: Buffer, event: &Event) -> Result<(), Error> {
    if event.value.serialized_size() > (LOGGER_ENTRY_MAX_LEN - 1 - 2 - 4 - 4 - 4) {
        return Err(Error::EventSize);
    }

    #[cfg(target_os = "android")]
    crate::logd::write_event(log_buffer, event);

    #[cfg(not(target_os = "android"))]
    println!("buffer: {:?}, event: {:?}", log_buffer, event);

    Ok(())
}
