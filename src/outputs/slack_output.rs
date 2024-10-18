use crate::monitor::{MonitorResult, Reporter};
use async_trait::async_trait;
use slack_morphism::prelude::*;
use url::Url;

pub struct SlackReporter<'a> {
    webhook_url: Url,
    formatter: &'a Formatter,
}

unsafe impl<'a> Sync for SlackReporter<'a> {}

type Formatter = dyn Fn(&MonitorResult) -> String + Sync;

impl<'a> SlackReporter<'a> {
    pub fn from_toml(config: toml::Table, formatter: &'a Formatter) -> Self {
        let webhook_url: Url = Url::parse(config["webhook_url"].as_str().unwrap()).unwrap();
        Self {
            webhook_url,
            formatter,
        }
    }
}

#[async_trait]
impl<'a> Reporter for SlackReporter<'a> {
    fn format(&self, output: &MonitorResult) -> String {
        (self.formatter)(output)
    }
    async fn report(&self, output: &MonitorResult) {
        dbg!(output);
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
    }
}
