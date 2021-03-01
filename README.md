![CI](https://github.com/flxo/logd-logger/workflows/CI/badge.svg)

logd-logger
===========
A library to simplify logging on Android.

The library provides a logd implementation for the API of 
[log](https://crates.io/crates/log).
Only one logging implementation may exist at runtime for rusts log API.
Logd-logger is configured once when initiating.
Messages are sent using the macros `error!`, `warn!`, `info!`, `debug!` and `trace!`.

## Android
In case logd-logger is compiled for Android, 
the log messages are sent to Android's logd socket.

## Other OS
For other Operating Systems the logging messages are sent to standard out.
These are displayed in logd format.

# Usage
First, add this to your Cargo.toml

```toml
[dependencies]
logd-logger = "0.1.0"
```

Next:
In the example the logging is initialized. First a [`Builder`] is created.
Then the log level is set. All messages with lower log level are dropped.
Then a tag is set which is added in front of each log message.
Next, the module name is set to be prepended.
After the configuration, it is tried to initialize the global logger.

Afterwards five different macros can be used for logging.
These are named after their log level.

```rust
use log::*;
use logd_logger;

fn main() {
    logd_logger::builder()
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

# Features
For use on Android it is possible to select whether all log messages should
be sent over a shared socket or whether each thread should manage its own
socket.
Default feature is a shared socket.
For thread locale sockets the feature flag `tls` needs to be set, and the
default features must be disabled with `--no-default-features`.

