use crate::monitor::{MonitorResult, Reporter};
use async_trait::async_trait;
use slack_morphism::prelude::*;
use tracing::event;
use tracing::Level as tLevel;
use url::Url;

pub struct SlackReporter {
    webhook_url: Url,
    renderer: upon::Engine<'static>,
}

impl SlackReporter {
    pub fn from_toml(config: &toml::Table) -> Result<Self, String> {
        let c = config["webhook_url"]
            .as_str()
            // this maps option to our expected result so we can ?
            .ok_or("missing Slack webhook_url config item")?;
        let webhook_url: Url =
            // same thing but wraps the existing error
            Url::parse(c).map_err(|e| format!("error parsing webhook_url ({0})", e))?;
        // dbg!(&webhook_url);

        let mut template = DEFAULT_TEMPLATE.to_string();
        if config.contains_key("template") {
            template = config["template"]
                .as_str()
                .ok_or("parse to convert Slack template top string")?
                .to_string();
        }
        let mut renderer = upon::Engine::new();
        renderer
            .add_template("t", template)
            .map_err(|e| format!("failed to register Slack template ({0})", e))?;
        Ok(Self {
            webhook_url,
            renderer,
        })
    }
    fn format(&self, output: &MonitorResult) -> Result<SlackMessageContent, String> {
        let body = self
            .renderer
            .template("t")
            .render(upon::value![res: output])
            .to_string()
            .map_err(|e| format!("error rendering Slack template {0}", e))?;
        Ok(
            SlackMessageContent::new().with_blocks(vec![SlackBlock::from(
                SlackSectionBlock::new().with_text(md!(body)),
            )]),
        )
    }
}
unsafe impl Send for SlackReporter {}
unsafe impl Sync for SlackReporter {}

#[async_trait]
impl Reporter for SlackReporter {
    async fn report(&self, output: &MonitorResult) {
        let slack_content = self.format(output);
        if let Err(e) = slack_content {
            event!(tLevel::ERROR, e);
            return;
        }
        // dbg!(&slack_message);
        let connector = SlackClientHyperConnector::new();
        match connector {
            Ok(c) => {
                // dbg!(slack_content.unwrap());
                let client = SlackClient::new(c);
                let res = client
                    .post_webhook_message(
                        &self.webhook_url,
                        &SlackApiPostWebhookMessageRequest::new(slack_content.unwrap()), // checked this already
                    )
                    .await;
                if let Err(e) = res {
                    let msg = format!("Slack webhook POST failed ({0})", e);
                    event!(tLevel::WARN, msg);
                }
                event!(tLevel::INFO, "processed monitor result via slack reporter");
            }
            Err(e) => {
                let msg = format!("failed to create Slack connector ({0})", e);
                event!(tLevel::WARN, msg);
            }
        }
    }
}

const DEFAULT_TEMPLATE: &str = "command: {{ res.target }}
args: {{ res.args }}
stdout: {{ res.stdout }}
stderr: {{ res.stderr }}
result: {{ res.status }}
duration {{ res.duration }} μs";
