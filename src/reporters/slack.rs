use crate::{
    monitor::MonitorResult,
    reporters::{util, Reporter},
};
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
    pub fn from_toml(config: &toml::Table) -> Result<Self, util::ConfigError> {
        let webhook_url = util::get_str_or_else(config, "webhook_url", None)?;
        let report_tmpl =
            util::get_str_or_else(config, "report_template", Some(DEF_REPORT_TEMPLATE))?;
        let clear_tmpl = util::get_str_or_else(config, "clear_template", Some(DEF_CLEAR_TEMPLATE))?;

        let mut renderer = upon::Engine::new();
        renderer
            .add_template("report", report_tmpl)
            .map_err(|e| format!("failed to register slack report template ({0})", e))?;
        renderer
            .add_template("clear", clear_tmpl)
            .map_err(|e| format!("failed to register slack clear template ({0})", e))?;
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
        // dbg!(content);
        // dbg!(&res);
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
    async fn report(&mut self, output: &MonitorResult) {
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
    async fn clear(&mut self, output: &MonitorResult) {
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

    fn get_state(&self) -> Option<Vec<u8>> {
        None
    }

    fn load_state(&mut self, _: Vec<u8>) {}
}

const DEF_REPORT_TEMPLATE: &str = "*Monitor: {{res.name}} [level: {{res.level_name}}*] 
*command:* {{ res.target }} 
*args:* {{ res.args }} 
*stdout:* {{ res.stdout }} 
*stderr:* {{ res.stderr }} 
*result:*{{ res.status }} 
*duration:* {{ res.duration }} μs";

const DEF_CLEAR_TEMPLATE: &str = "*Monitor: {{res.name}} returned to baseline*";
