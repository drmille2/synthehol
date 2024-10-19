mod slack_output;

use crate::monitor::{Reporter, ReporterArgs};

use tracing::event;
use tracing::Level as tLevel;

pub fn initialize_slack(
    args: &ReporterArgs,
) -> Result<Box<dyn Reporter + Send + Sync + 'static>, String> {
    let out = Box::new(slack_output::SlackReporter::from_toml(args, &|x| {
        let name = x.name.clone();
        let result = x.status;
        let stdout = x.stdout.clone();
        let stderr = x.stderr.clone();
        format!("Result for {name}:\nstatus: {result}\nstdout: {stdout}\nstderr: {stderr}")
    })?);
    event!(tLevel::INFO, "initialized slack reporter");
    Ok(out)
}
