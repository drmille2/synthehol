mod slack_output;

use crate::monitor::{Reporter, ReporterArgs};

pub fn initialize_slack(args: ReporterArgs) -> Box<dyn Reporter> {
    Box::new(slack_output::SlackReporter::from_toml(args, &|x| {
        let name = x.name.clone();
        let result = x.status;
        let stdout = x.stdout.clone();
        let stderr = x.stderr.clone();
        format!("Result for {name}:\nstatus: {result}\nstdout: {stdout}\nstderr: {stderr}")
    }))
}
