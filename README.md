<!-- cargo-sync-readme start -->

# `android-logd-logger`

This logger writes logs to the Android `logd`, a system service with
multiple ringbuffers for logs and evens. This is normally done
via `liblog` (a native Android lib). Instead of using `liblog`, this crate
writes directly to the `logd` socket with the trivial protocol below.
This logger is written in pure Rust without any need for ffi.

[log]: https://docs.rs/log/*/log/
[`error!`]: https://docs.rs/log/*/log/macro.error.html
[`warn!`]: https://docs.rs/log/*/log/macro.warn.html
[`info!`]: https://docs.rs/log/*/log/macro.info.html
[`debug!`]: https://docs.rs/log/*/log/macro.debug.html
[`trace!`]: https://docs.rs/log/*/log/macro.trace.html

On non Android system the log output is printed to stdout in the default
format of `logcat`.

# Usage

Add this to your Cargo.toml

```toml
[dependencies]
android-logd-logger = "0.1.2"
```
Initialize the logger with a fixed `tag` and the module path included
in the log payload.

```rust

fn main() {
    android_logd_logger::builder()
        .parse_filters("debug")
        .tag("log_tag")
        .prepend_module(true)
        .init();

    trace!("trace message: is not logged");
    debug!("debug message");
    info!("info message");
    warn!("warn message");
    error!("error message");
}
```

To write android logd "events" use `event` or `event_now`, e.g:

```rust
android_logd_logger::event_now(1, "hui").unwrap();
```

# Configuration

Writing to the logd socket is a single point of synchronization for threads.
The `android-logd-logger` can be configured with the `tls` feature to maintain
one socket per thread *or* use a single socket for the whole process.
Use the features `tls` if you want less interference between threads but pay
for one connection per thread.

# License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

<!-- cargo-sync-readme end -->
