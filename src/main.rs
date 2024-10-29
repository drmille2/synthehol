mod monitor;
mod reporters;

use crate::reporters::slack_reporter::SlackReporter;
use crate::reporters::splunk_reporter::SplunkReporter;

use std::fs;
// use std::future;
use std::str::FromStr;

use clap::Parser;
use serde::Deserialize;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::Level;

use tokio_util::task::TaskTracker;

#[derive(Parser, Debug)]
#[command(
    author = "David Miller",
    version = "v0.1.0",
    about = "Synthehol (easily replicable synthetic monitoring)"
)]

struct Cli {
    #[clap(short, long)]
    config: String,
}

#[derive(Deserialize, Debug)]
struct Config {
    log_level: Option<String>,
    monitor: Vec<monitor::MonitorArgs>,
    splunk: Option<monitor::ReporterArgs>,
    slack: Option<monitor::ReporterArgs>,
    // pagerduty: Option<monitor::ReporterArgs>,
}

fn parse_config(path: String) -> Config {
    let input = &fs::read_to_string(path).expect("failed to read configuration file");
    toml::from_str(input).expect("failed to parse configuration file")
}

#[tokio::main]
async fn main() {
    let cli_args = Cli::parse();
    let config = parse_config(cli_args.config);

    let lev = Level::from_str(&config.log_level.unwrap_or(String::from("info")))
        .expect("invalid log level");
    tracing_subscriber::fmt()
        .with_max_level(lev)
        .with_thread_ids(true)
        .init();
    // dbg!(&config);

    // parse all our monitor configs
    // there's some duplicated work with the reporters being
    // initialized separately for each monitor and copied here
    let mut mons = Vec::new();
    for m in config.monitor {
        info!("config parsed for monitor: {}", m.name);
        let mut mon = monitor::Monitor::from_args(m);

        // initialize and register slack reporter if configured, panics on failure
        if let Some(r) = &config.slack {
            let slack =
                Box::new(SlackReporter::from_toml(r).expect("failed to initialize slack reporter"));
            mon.register_reporter("slack", slack);
        }

        if let Some(r) = &config.splunk {
            let splunk = Box::new(
                SplunkReporter::from_toml(r).expect("failed to initialize splunk reporter"),
            );
            mon.register_reporter("splunk", splunk);
        }

        mons.push(mon);
    }

    let token = CancellationToken::new();

    // spawn monitor loops and do nothing
    let tracker = TaskTracker::new();
    for mut m in mons {
        let cancel = token.clone();
        tracker.spawn(async move { m.start(cancel).await });
        // tracker.spawn(async move {
        //     tokio::select! {
        //         _ = cloned_token.cancelled() => {
        //         }
        //         _ = m.start() => {
        //         }
        //     }
        // });
    }
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("Interrupt received, shutting down...");
            token.cancel()
        }
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
        }
    }
    tracker.close();
    tracker.wait().await;

    // let future = future::pending();
    // let () = future.await;
}
