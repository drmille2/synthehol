mod config;
mod db;
mod monitor;
mod reporters;
mod target;

use crate::config::parse_config;

use std::str::FromStr;

use clap::Parser;
use tokio::signal::unix::{signal, SignalKind};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::Level;

use tokio_util::task::TaskTracker;

#[derive(Parser, Debug)]
#[command(
    author = "David Miller",
    version = "v0.2.0",
    about = "Synthehol (easily replicable synthetic monitoring)"
)]
struct Cli {
    #[clap(short, long)]
    config: String,
}

#[tokio::main]
async fn main() {
    let cli_args = Cli::parse();
    let config =
        parse_config(&cli_args.config).expect("failed to parse provided configuration path");

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
    let monitors = config.monitor.expect("no monitors defined, exiting");
    for m in monitors {
        info!("config parsed for monitor: {}", m.name);
        let mut mon = m.build();

        mon.register_db(db);

        // initialize and register slack reporter if configured, panics on failure
        if let Some(r) = &config.slack {
            let slack = r.clone().build();
            if let Err(e) = slack {
                panic!("slack reported failed to initilize ({})", e)
            }
            let slack = Box::new(slack.unwrap());
            mon.register_reporter("slack", slack);
            info!("slack reporter registered");
        }

        // initialize and register splunk reporter if configured, panics on failure
        if let Some(r) = &config.splunk {
            let splunk = Box::new(r.clone().build());
            mon.register_reporter("splunk", splunk);
            info!("splunk reporter registered");
        }

        // initialize and register pagerduty reporter if configured, panics on failure
        if let Some(r) = &config.pagerduty {
            let pagerduty = r.clone().build();
            if let Err(e) = pagerduty {
                panic!("pagerduty reported failed to initilize ({})", e)
            }
            let pagerduty = Box::new(pagerduty.unwrap());
            mon.register_reporter("pagerduty", pagerduty);
            info!("pagerduty reporter registered");
        }

        // initialize and register postgresql reporter if configured, panics on failure
        if let Some(r) = &config.postgresql {
            let postgresql = Box::new(
                r.clone()
                    .build()
                    .await
                    .expect("failed to initialize postgresql reporter"),
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
