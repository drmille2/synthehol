mod db;
mod monitor;
mod reporters;

use crate::reporters::slack_reporter::SlackReporter;
use crate::reporters::splunk_reporter::SplunkReporter;

use std::fs;
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
    use_db_persistence: Option<bool>,
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
    let db = if config.use_db_persistence.unwrap_or(true) {
        db::SynthDb::new(Some("./synthehol.db"))
            .await
            .expect("failed to open sqlite database")
    } else {
        db::SynthDb::new(None)
            .await
            .expect("failed to open sqlite database")
    };
    db.initialize_db()
        .await
        .expect("failed to initialize database");
    let db = Box::leak(Box::new(db));

    // parse all our monitor configs
    // there's some duplicated work with the reporters being
    // initialized separately for each monitor and copied here
    let mut mons = Vec::new();
    for m in config.monitor {
        info!("config parsed for monitor: {}", m.name);
        let mut mon = monitor::Monitor::from_args(m);

        mon.register_db(db);

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

    // spawn monitors with cancellation token for to trigger
    // a graceful(ish) shutdown after an interrupt, and use
    // a tracker to wait for the tasks to finish before exiting
    let token = CancellationToken::new();
    let tracker = TaskTracker::new();
    for mut m in mons {
        let cancel = token.clone();
        tracker.spawn(async move { m.start(cancel).await });
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
}
