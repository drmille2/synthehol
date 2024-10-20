mod slack_reporter;

use crate::monitor::{Reporter, ReporterArgs};

use slack_morphism::prelude::*;
use tracing::event;
use tracing::Level as tLevel;

pub fn initialize_slack(
    args: &ReporterArgs,
) -> Result<Box<dyn Reporter + Send + Sync + 'static>, String> {
    let out = Box::new(slack_reporter::SlackReporter::from_toml(args, &|x| {
        SlackMessageContent::new().with_blocks(slack_blocks![
            some_into(SlackSectionBlock::new()),
            some_into(SlackDividerBlock::new()),
            some_into(SlackHeaderBlock::new(pt!(format!(
                "({0}) Output for monitor {1}",
                x.level_name, x.name
            )))),
            some_into(SlackDividerBlock::new()),
            some_into(SlackContextBlock::new(slack_blocks![some(md!(format!(
                "STDOUT: {0}\n\
                             STDERR: {1}\n\
                             RESULT CODE: {2}\n\
                             DURATION: {3}",
                x.stdout.clone(),
                x.stderr.clone(),
                x.status,
                x.duration
            )))])),
            some_into(SlackActionsBlock::new(slack_blocks![some_into(
                SlackBlockButtonElement::new(
                    "simple-message-button".into(),
                    pt!("Simple button text")
                )
            )]))
        ])

        // let name = x.name.clone();
        // let result = x.status;
        // let stdout = x.stdout.clone();
        // let stderr = x.stderr.clone();
        // let blocks = vec![
        //     SlackBlock::Header
        // ];
        // let slack_message =
        //     format!("Result for {name}:\nstatus: {result}\nstdout: {stdout}\nstderr: {stderr}");
        // SlackMessageContent::new().with_text(slack_message)
    })?);
    event!(tLevel::INFO, "initialized slack reporter");
    Ok(out)
}
