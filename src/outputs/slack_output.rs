use crate::monitor::{MonitorResult, Reporter};
use async_trait::async_trait;
use slack_morphism::prelude::*;
use tracing::event;
use tracing::Level as tLevel;
use url::Url;

pub struct SlackReporter<'a> {
    webhook_url: Url,
    formatter: &'a Formatter,
}

type Formatter = dyn Fn(&MonitorResult) -> String + Sync;

impl<'a> SlackReporter<'a> {
    pub fn from_toml(config: &toml::Table, formatter: &'a Formatter) -> Result<Self, String> {
        let c = config["webhook_url"]
            .as_str()
            // this maps option to our expected result so we can ?
            .ok_or("missing Slack webhook_url config item")?;
        let webhook_url: Url =
            // same thing but wraps the existing error
            Url::parse(c).map_err(|e| format!("error parsing webhook_url {0}", e))?;
        dbg!(&webhook_url);
        Ok(Self {
            webhook_url,
            formatter,
        })
    }
    fn format(&self, output: &MonitorResult) -> String {
        (self.formatter)(output)
    }
}
unsafe impl Send for SlackReporter<'static> {}
unsafe impl Sync for SlackReporter<'static> {}

#[async_trait]
impl Reporter for SlackReporter<'static> {
    async fn report(&self, output: &MonitorResult) {
        let slack_message = self.format(output);
        dbg!(&slack_message);
        let connector = SlackClientHyperConnector::new();
        match connector {
            Ok(c) => {
                let client = SlackClient::new(c);
                let res = client
                    .post_webhook_message(
                        &self.webhook_url,
                        &SlackApiPostWebhookMessageRequest::new(
                            SlackMessageContent::new().with_text(slack_message),
                        ),
                    )
                    .await;
                if let Err(e) = res {
                    let msg = format!("Slack webhook POST failed ({0})", e);
                    event!(tLevel::WARN, msg);
                }
                // .unwrap();
                event!(tLevel::INFO, "processed monitor result via slack reporter");
            }
            Err(e) => {
                let msg = format!("failed to create Slack connector ({0})", e);
                event!(tLevel::WARN, msg);
            }
        }
    }
}
