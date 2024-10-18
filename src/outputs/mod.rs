mod slack_output;

use crate::monitor::{Reporter, ReporterArgs};

fn initialize(args: Vec<ReporterArgs>) -> Vec<Box<dyn Reporter>> {
    let mut out = Vec::new();
    let slack_args = args[0].clone();
    let slack: Box<dyn Reporter> =
        Box::new(slack_output::SlackReporter::from_toml(slack_args, &|x| {
            let name = x.name.clone();
            let result = x.status.clone();
            let stdout = x.stdout.clone();
            let stderr = x.stderr.clone();
            format!("Result for {name}:\nstatus: {result}\nstdout: {stdout}\nstderr: {stderr}")
        }));
    out.push(slack);
    out
}
