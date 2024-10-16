use crate::monitor::{MonResult, Reporter};
use slack_morphism::prelude::*;
use url::Url;

pub struct SlackReporter<'a> {
    webhook_url: Url,
    formatter: &'a dyn Fn(MonResult) -> String,
}

impl<'a> SlackReporter<'a> {
    pub fn new(config: toml::Table, formatter: &'a dyn Fn(MonResult) -> String) -> Self {
        let webhook_url: Url = Url::parse(config["webhook_url"].as_str().unwrap()).unwrap();

        Self {
            webhook_url,
            formatter,
        }
    }
    async fn report(&self, output: MonResult) {
        let slack_message = self.format(output);
        let client = SlackClient::new(SlackClientHyperConnector::new().unwrap());
        let _ = client
            .post_webhook_message(
                &self.webhook_url,
                &SlackApiPostWebhookMessageRequest::new(
                    SlackMessageContent::new().with_text(slack_message),
                ),
            )
            .await;
    }
}

impl<'a> Reporter for SlackReporter<'a> {
    fn format(&self, output: MonResult) -> String {
        let formatter = self.formatter;
        formatter(output)
    }
}
