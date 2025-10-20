# `android-logd-logger`

[![Crates.io][crates-badge]][crates-url]
[![Build Status][actions-badge]][actions-url]
[![Docs][docs-badge]][docs-url]

[docs-badge]: https://docs.rs/android-logd-logger/badge.svg
[docs-url]: https://docs.rs/android-logd-logger
[crates-badge]: https://img.shields.io/crates/v/android-logd-logger.svg
[crates-url]: https://crates.io/crates/android-logd-logger
[actions-badge]: https://github.com/flxo/android-logd-logger/workflows/CI/badge.svg
[actions-url]: https://github.com/flxo/android-logd-logger/actions?query=workflow%3ACI+branch%3Amaster

This logger writes logs to the Android `logd`, a system service with
multiple ring buffers for logs and events. This is normally done
via `liblog` (a native Android library). Instead of using `liblog`, this crate
writes directly to the `logd` socket with a simple protocol.
This logger is written in pure Rust without any need for FFI.

On non Android system the log output is printed to stdout in the default
format of `logcat`.

## Usage

Add this to your Cargo.toml

```toml
[dependencies]
android-logd-logger = "0.4.3"
```

Initialize the logger with a fixed `tag` and the module path included
in the log payload.

```rust
use log::{debug, error, info, trace, warn};

fn main() {
    android_logd_logger::builder()
        .parse_filters("debug")
        .tag("MyApp")
        .prepend_module(true)
        .init();

    trace!("trace message: is not logged");
    debug!("debug message");
    info!("info message");
    warn!("warn message");
    error!("error message");
}
```

## Runtime Configuration

The logger can be reconfigured at runtime using the returned `Logger` handle:

```rust
use log::LevelFilter;

fn main() {
    let logger = android_logd_logger::builder()
        .tag("MyApp")
        .init();

    // Change settings at runtime
    logger.tag("NewTag");
    logger.filter_level(LevelFilter::Warn);
}
```

To write Android logd "events" use `write_event_now`, e.g:

```rust
android_logd_logger::write_event_now(1, "test").unwrap();
```

## Configuration

Writing to the logd socket is a single point of synchronization for threads.
The `android-logd-logger` can be configured with the `tls` feature to maintain
one socket per thread *or* use a single socket for the whole process.
Use the features `tls` if you want less interference between threads but pay
for one connection per thread.

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
