mod slack_reporter;

use crate::monitor::{Reporter, ReporterArgs};

use slack_morphism::prelude::*;
use tracing::event;
use tracing::Level as tLevel;

pub fn initialize_slack(
    args: &ReporterArgs,
) -> Result<Box<dyn Reporter + Send + Sync + 'static>, String> {
    let out = Box::new(slack_reporter::SlackReporter::from_toml(args, &|x| {
        let body = format!(
            "command: {0}
            args: {1:?}
            stderr: {2}
            result: {3}
            duration {4}",
            x.target, x.args, x.stderr, x.status, x.duration
        );
        let header = format!("({0}) Output for monitor {1}", x.level_name, x.name);
        SlackMessageContent::new().with_blocks(vec![
            SlackBlock::from(SlackHeaderBlock::new(pt!(header))),
            SlackBlock::from(SlackDividerBlock::new()),
            SlackBlock::from(SlackSectionBlock::new().with_text(md!(body))),
        ])
    })?);
    event!(tLevel::INFO, "initialized slack reporter");
    Ok(out)
}
