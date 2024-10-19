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
    pub fn from_toml(config: &toml::Table, formatter: &'a Formatter) -> Self {
        let webhook_url: Url = Url::parse(config["webhook_url"].as_str().unwrap()).unwrap();
        dbg!(&webhook_url);
        Self {
            webhook_url,
            formatter,
        }
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
        let client = SlackClient::new(SlackClientHyperConnector::new().unwrap());
        client
            .post_webhook_message(
                &self.webhook_url,
                &SlackApiPostWebhookMessageRequest::new(
                    SlackMessageContent::new().with_text(slack_message),
                ),
            )
            .await
            .unwrap();
        event!(tLevel::INFO, "processed monitor result via slack reporter");
    }
}
