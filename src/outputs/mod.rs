mod slack_output;

use crate::monitor::{Reporter, ReporterArgs};

fn initialize<T>(args: Vec<ReporterArgs>) -> Vec<T>
where
    T: Reporter,
{
    let mut out = Vec::new();
    let slack = slack_output::SlackReporter::from_toml(args[0], &|x| {
        let target = x.target.path;
        let result = x.result;
        let stdout = x.stdout;
        let stderr = x.stderr;
        format!("Result for {target}:\nstatus: {result}\nstdout: {stdout}\nstderr: {stderr}")
    });
    out.push(slack);
    out
}
