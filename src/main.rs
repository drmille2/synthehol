mod db;
mod monitor;
mod reporters;

use crate::reporters::pagerduty::PagerdutyReporter;
use crate::reporters::postgresql::PostgresqlReporter;
use crate::reporters::slack::SlackReporter;
use crate::reporters::splunk::SplunkReporter;

use std::fs;
use std::str::FromStr;

use clap::Parser;
use serde::Deserialize;
use tokio::signal::unix::{signal, SignalKind};
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
    pagerduty: Option<monitor::ReporterArgs>,
    postgresql: Option<monitor::ReporterArgs>,
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
        let mut mon = m.build();

        mon.register_db(db);

        // initialize and register slack reporter if configured, panics on failure
        if let Some(r) = &config.slack {
            let slack =
                Box::new(SlackReporter::from_toml(r).expect("failed to initialize slack reporter"));
            mon.register_reporter("slack", slack);
            info!("slack reporter registered");
        }

        // initialize and register splunk reporter if configured, panics on failure
        if let Some(r) = &config.splunk {
            let splunk = Box::new(
                SplunkReporter::from_toml(r).expect("failed to initialize splunk reporter"),
            );
            mon.register_reporter("splunk", splunk);
            info!("splunk reporter registered");
        }

        // initialize and register pagerduty reporter if configured, panics on failure
        if let Some(r) = &config.pagerduty {
            let pagerduty = Box::new(
                PagerdutyReporter::from_toml(r).expect("failed to initialize pagerduty reporter"),
            );
            mon.register_reporter("pagerduty", pagerduty);
            info!("pagerduty reporter registered");
        }

        // initialize and register postgresql reporter if configured, panics on failure
        if let Some(r) = &config.postgresql {
            let postgresql = Box::new(
                PostgresqlReporter::from_toml(r)
                    .await
                    .expect("failed to initialize pagerduty reporter"),
            );
            mon.register_reporter("postgresql", postgresql);
            info!("postgresql reporter registered");
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
    let mut sigint = signal(SignalKind::interrupt()).expect("error handling interrupt signal");
    let mut sigterm = signal(SignalKind::interrupt()).expect("error handling terminate signal");

    tokio::select! {
        _ = sigint.recv() => {
            info!("Interrupt received, shutting down...");
            token.cancel()
            },
        _ = sigterm.recv() => {
            info!("Terminate received, shutting down...");
            token.cancel()
        }
    }
    tracker.close();
    tracker.wait().await;
}
