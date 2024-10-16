mod monitor;
mod outputs;

use std::fs;

use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(
    author = "David Miller",
    version = "v0.1.0",
    about = "Synthehol (easily replicated synthetic monitoring)"
)]

struct Cli {
    #[clap(short, long)]
    config: String,
}

#[derive(Deserialize, Debug)]
struct MonitorArgs {
    name: String,
    interval: u32,
    level: Vec<LevelArgs>,
    target: TargetArgs,
}

#[derive(Deserialize, Debug)]
struct LevelArgs {
    name: String,
    errors_to_escalate: Option<u32>,
    outputs: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct TargetArgs {
    path: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
}

#[derive(Deserialize, Debug)]
struct SplunkArgs {
    index: String,
    hec_token: String,
    endpoint: String,
}

#[derive(Deserialize, Debug)]
struct SlackArgs {
    api_key: String,
    endpoint: String,
}

#[derive(Deserialize, Debug)]
struct PagerdutyArgs {
    api_key: String,
    endpoint: String,
}

#[derive(Deserialize, Debug)]
struct Config {
    monitor: MonitorArgs,
    splunk: Option<SplunkArgs>,
    slack: Option<SlackArgs>,
    pagerduty: Option<PagerdutyArgs>,
}

fn parse_config(path: String) -> Config {
    let input = &fs::read_to_string(path).unwrap();
    toml::from_str(input).unwrap()
}

fn main() {
    let cli_args = Cli::parse();
    dbg!(parse_config(cli_args.config));
}
