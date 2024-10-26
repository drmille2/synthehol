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

        let mut report_tmpl = DEF_REPORT_TEMPLATE.to_string();
        if config.contains_key("template") {
            report_tmpl = config["template"]
                .as_str()
                .ok_or("parse to convert Slack template top string")?
                .to_string();
        }
        let clear_tmpl = DEF_CLEAR_TEMPLATE.to_string();
        let mut renderer = upon::Engine::new();
        renderer
            .add_template("report", report_tmpl)
            .map_err(|e| format!("failed to register Slack template ({0})", e))?;
        renderer
            .add_template("clear", clear_tmpl)
            .map_err(|e| format!("failed to register Slack template ({0})", e))?;
        Ok(Self {
            webhook_url,
            renderer,
        })
    }
    fn format(
        &self,
        template: &str,
        output: &MonitorResult,
    ) -> Result<SlackMessageContent, String> {
        let body = self
            .renderer
            .template(template)
            .render(upon::value![res: output])
            .to_string()
            .map_err(|e| format!("error rendering Slack template {0}", e))?;
        Ok(
            SlackMessageContent::new().with_blocks(vec![SlackBlock::from(
                SlackSectionBlock::new().with_text(md!(body)),
            )]),
        )
    }

    async fn send(&self, content: SlackMessageContent) {
        let connector = SlackClientHyperConnector::new();
        match connector {
            Ok(c) => {
                // dbg!(slack_content.unwrap());
                let client = SlackClient::new(c);
                let res = client
                    .post_webhook_message(
                        &self.webhook_url,
                        &SlackApiPostWebhookMessageRequest::new(content),
                    )
                    .await;
                if let Err(e) = res {
                    event!(tLevel::WARN, "sending to slack webhook failed ({})", e);
                }
                event!(tLevel::DEBUG, "processed monitor result via slack reporter");
            }
            Err(e) => {
                event!(tLevel::WARN, "failed to create slack connector ({})", e);
            }
        }
    }
}

#[async_trait]
impl Reporter for SlackReporter {
    async fn report(&self, output: &MonitorResult) {
        let slack_content = self.format("report", output);
        match slack_content {
            Ok(slack_content) => {
                self.send(slack_content).await;
            }
            Err(e) => {
                event!(tLevel::ERROR, e);
            }
        }
        // if let Err(e) = slack_content {
        //     event!(tLevel::ERROR, e);
        //     return;
        // }
        // self.send(slack_content.unwrap()).await;
    }

    async fn clear(&self, output: &MonitorResult) {
        let slack_content = self.format("clear", output);
        match slack_content {
            Ok(slack_content) => {
                event!(tLevel::INFO, "cleared slack alert");
                self.send(slack_content).await;
            }
            Err(e) => {
                event!(tLevel::ERROR, e);
            }
        }
        // if let Err(e) = slack_content {
        //     event!(tLevel::ERROR, e);
        //     return;
        // }
        // event!(tLevel::INFO, "cleared slack alert");
        // self.send(slack_content.unwrap()).await;
    }
}

const DEF_REPORT_TEMPLATE: &str = "### Monitor {{res.name}} output (level = {{res.level_name}}) 
command: {{ res.target }} 
args: {{ res.args }} 
stdout: {{ res.stdout }} 
stderr: {{ res.stderr }} 
result: {{ res.status }} 
duration: {{ res.duration }} μs";

const DEF_CLEAR_TEMPLATE: &str = "### Monitor {{res.name}} returned to baseline";
