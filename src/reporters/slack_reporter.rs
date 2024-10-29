use crate::monitor::{MonitorResult, Reporter};
use async_trait::async_trait;
use serde::Serialize;
use tracing::{debug, instrument, warn};

#[derive(Debug)]
pub struct SlackReporter {
    webhook_url: String,
    renderer: upon::Engine<'static>,
}

#[derive(Serialize, Debug)]
struct SlackMessage {
    blocks: Vec<SlackSectionBlock>,
}

#[derive(Serialize, Debug)]
struct SlackTextBlock {
    r#type: String,
    text: String,
}

#[derive(Serialize, Debug)]
struct SlackSectionBlock {
    r#type: String,
    text: SlackTextBlock,
}

impl SlackReporter {
    pub fn from_toml(config: &toml::Table) -> Result<Self, String> {
        let webhook_url = config["webhook_url"]
            .as_str()
            // this maps option to our expected result so we can ?
            .ok_or("missing Slack webhook_url config item")?;
        let webhook_url = String::from(webhook_url);
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
            .map_err(|e| format!("failed to register slack template ({0})", e))?;
        renderer
            .add_template("clear", clear_tmpl)
            .map_err(|e| format!("failed to register slack template ({0})", e))?;
        Ok(Self {
            webhook_url,
            renderer,
        })
    }

    #[instrument]
    fn format(&self, template: &str, output: &MonitorResult) -> Result<SlackMessage, String> {
        let body = self
            .renderer
            .template(template)
            .render(upon::value![res: output])
            .to_string()
            .map_err(|e| format!("error rendering slack template {0}", e))?;
        let out = SlackMessage {
            blocks: vec![SlackSectionBlock {
                r#type: String::from("section"),
                text: SlackTextBlock {
                    r#type: String::from("mrkdwn"),
                    text: body,
                },
            }],
        };
        Ok(out)
    }

    #[instrument]
    async fn send(&self, content: &SlackMessage) {
        let client = reqwest::Client::new();
        let res = client
            .post(&self.webhook_url)
            .header("Content-Type", "application/json")
            .json(content);
        dbg!(content);
        dbg!(&res);
        let res = res.send().await;
        match res {
            Ok(r) => debug!("slack report successful ({})", r.status()),
            Err(e) => warn!("slack report failed ({})", e),
        }
    }
}

#[async_trait]
impl Reporter for SlackReporter {
    #[instrument]
    async fn report(&self, output: &MonitorResult) {
        let slack_content = self.format("report", output);
        match slack_content {
            Ok(slack_content) => {
                self.send(&slack_content).await;
            }
            Err(e) => {
                warn!(e);
            }
        }
    }

    #[instrument]
    async fn clear(&self, output: &MonitorResult) {
        let slack_content = self.format("clear", output);
        match slack_content {
            Ok(slack_content) => {
                debug!("slack alert cleared for monitor {}", output.name);
                self.send(&slack_content).await;
            }
            Err(e) => {
                warn!(e);
            }
        }
    }
}

const DEF_REPORT_TEMPLATE: &str = "*Monitor: {{res.name}} [level: {{res.level_name}}*] 
*command:* {{ res.target }} 
*args:* {{ res.args }} 
*stdout:* {{ res.stdout }} 
*stderr:* {{ res.stderr }} 
*result:*{{ res.status }} 
*duration:* {{ res.duration }} μs";

const DEF_CLEAR_TEMPLATE: &str = "*Monitor: {{res.name}} returned to baseline*";
