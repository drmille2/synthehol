use crate::monitor::{MonitorResult, Reporter};
use slack_morphism::prelude::*;
use url::Url;

pub struct SlackReporter<'a> {
    webhook_url: Url,
    formatter: &'a dyn Fn(MonitorResult) -> String,
}

impl<'a> SlackReporter<'a> {
    pub fn from_toml(config: toml::Table, formatter: &'a dyn Fn(MonitorResult) -> String) -> Self {
        let webhook_url: Url = Url::parse(config["webhook_url"].as_str().unwrap()).unwrap();
        Self {
            webhook_url,
            formatter,
        }
    }
}

impl<'a> Reporter for SlackReporter<'a> {
    fn format(&self, output: MonitorResult) -> String {
        (self.formatter)(output)
    }
    async fn report(&self, output: MonitorResult) {
        let slack_message = self.format(output);
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

pub fn default_formatter(res: MonitorResult) -> String {
    let target = res.target.path;
    let result = res.result;
    let stdout = res.stdout;
    let stderr = res.stderr;
    format!("Result for {target}:\nstatus: {result}\nstdout: {stdout}\nstderr: {stderr}")
}
