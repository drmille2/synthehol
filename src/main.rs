mod monitor;
mod reporters;

use crate::reporters::slack_reporter::SlackReporter;
use crate::reporters::splunk_reporter::SplunkReporter;

use std::fs;
use std::str::FromStr;

use clap::Parser;
use serde::Deserialize;
use tokio::signal;
use tokio_rusqlite::Connection;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
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

async fn open_db(path: Option<&str>) -> Result<Connection, tokio_rusqlite::Error> {
    let db = match path {
        Some(path) => Connection::open(path).await?,
        None => Connection::open_in_memory().await?,
    };

    // TODO: there's a better way to handle the table creation errors
    // create results table if it doesn't exist
    debug!("setting sqlite pragmas...");
    db.call(|db| {
        db.execute("PRAGMA cache_size = -4096", [])
            .map_err(|e| e.into())
    })
    .await
    .unwrap_or_else(|e| {
        error!("failed to set pragmas ({})", e);
        0
    });
    debug!("attempting to create results table...");
    db.call(|db| {
        db.execute(
            "CREATE TABLE IF NOT EXISTS results (
                id    INTEGER PRIMARY KEY,
                monitor_name  TEXT NOT NULL,
                level_name TEXT NOT NULL,
                start_time INTEGER NOT NULL,
                target_name   TEXT NOT NULL,
                args TEXT,
                stdout TEXT,
                stderr TEXT,
                duration INTEGER,
                status INTEGER
            )",
            [],
        )
        .map_err(|e| e.into())
    })
    .await
    .expect("error creating results table");

    // create monitor_state table if it doesn't exist
    debug!("attempting to create monitor_state table...");
    db.call(|db| {
        db.execute(
            "CREATE TABLE IF NOT EXISTS monitor_state (
                id INTEGER PRIMARY KEY,
                name  TEXT UNIQUE,
                level_index INTEGER NOT NULL,
                failure_tally INTEGER NOT NULL,
                success_tally INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| e.into())
    })
    .await
    .expect("error creating monitor_state table");

    // create reporter_state table if it doesn't exist
    debug!("attempting to create reporter_state table...");
    db.call(|db| {
        db.execute(
            "CREATE TABLE IF NOT EXISTS reporter_state (
                id INTEGER PRIMARY KEY,
                name TEXT,
                monitor_name TEXT,
                state BLOB
            )",
            [],
        )
        .map_err(|e| e.into())
    })
    .await
    .expect("error creating reporter_state table");

    // add unique reporter state index
    db.call(|db| {
        db.execute(
            "CREATE UNIQUE INDEX idx_reporter_state_name_monitor_name 
                ON reporter_state (name, monitor_name);",
            [],
        )
        .map_err(|e| e.into())
    })
    .await
    .expect("error creating reporter_state table");

    Ok(db)
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
        Box::leak(Box::new(
            open_db(Some("./synthehol.db"))
                .await
                .expect("failed to open sqlite database"),
        ))
    } else {
        Box::leak(Box::new(
            open_db(None).await.expect("failed to open sqlite database"),
        ))
    };

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
