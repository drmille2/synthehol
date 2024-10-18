mod monitor;
mod outputs;

use std::fs;

use clap::Parser;
use serde::Deserialize;

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
    monitor: monitor::MonitorArgs,
    splunk: Option<monitor::ReporterArgs>,
    slack: Option<monitor::ReporterArgs>,
    pagerduty: Option<monitor::ReporterArgs>,
}

fn parse_config(path: String) -> Config {
    let input = &fs::read_to_string(path).unwrap();
    toml::from_str(input).unwrap()
}

#[tokio::main]
async fn main() {
    let cli_args = Cli::parse();
    let config = parse_config(cli_args.config);
    dbg!(&config);
    let mut mon = monitor::Monitor::from_args(config.monitor);
    if let Some(r) = config.slack {
        let slack = outputs::initialize_slack(r);
        mon.register_reporter("Slack", slack);
    }
    mon.start().await;
}
