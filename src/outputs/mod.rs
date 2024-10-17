mod slack_output;

use crate::monitor::{Reporter, ReporterArgs};

fn initialize<'a>(args: Vec<ReporterArgs>) -> Vec<Box<dyn Reporter>> {
    let mut out = Vec::new();
    let slack_args = args[0].clone();
    let slack: Box<dyn Reporter> =
        Box::new(slack_output::SlackReporter::from_toml(slack_args, &|x| {
            let target = x.target.path;
            let result = x.result;
            let stdout = x.stdout;
            let stderr = x.stderr;
            format!("Result for {target}:\nstatus: {result}\nstdout: {stdout}\nstderr: {stderr}")
        }));
    out.push(slack);
    out
}
