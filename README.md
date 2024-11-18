## Synthehol: Easily replicable synthetic monitoring

Synthehol is an application that makes it possible to turn any existing script or other executable into a synthetic monitor with a simple configuration file. Built in Rust, it provides a light-weight binary with a small footprint in order to run the configured monitor scripts at defined intervals, record their results and escalate between customized error levels. Each error level can be assigned a reporters that will handle the output (e.g. the Pagerduty reporter will open an event once escalated and close it once the monitor returns to baseline). 

### Features

#### Modular reporters

Supports the development of new reporters as modules. Reporters implement `report` and `clear` methods to handle the creation and resolution of incidents and `get_state` and `load_state` if they need to maintain internal state to function.

#### Local persistence

Sqlite persistence to disk is enabled by default to persist monitor & reporter state so data isn't lost on restart. Disk persistence can be disabled through config.

#### Async execution


Built in async Rust to be fast and reliable, Synthehol uses the tokio runtime for executing monitoring targets along with all db & network I/O. 

#### Small footprint

In release mode, Synthehol is an 8M binary with ~16M overhead and 2-3M of memory usage per configured monitor (varies depending on the size of monitor outputs). Perfect for deploying many long-running monitors to keep track of your services and infrastructure.

### Getting Started

#### Installation

Either download the latest binary for your architecture from the releases page, or compile from source using cargo.

#### Configuration

See `example.toml` for information on available configuration options.
