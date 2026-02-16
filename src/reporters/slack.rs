use crate::{monitor::MonitorResult, reporters::Reporter};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

#[derive(Debug)]
pub struct SlackReporter {
    renderer: upon::Engine<'static>,
    webhook_url: String,
    report_tmpl: String,
    clear_tmpl: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct SlackReporterArgs {
    webhook_url: String,
    report_tmpl: Option<String>,
    clear_tmpl: Option<String>,
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

impl SlackReporterArgs {
    pub fn build(self) -> Result<SlackReporter, upon::Error> {
        let mut r = SlackReporter {
            renderer: upon::Engine::new(),
            webhook_url: self.webhook_url,
            report_tmpl: self.report_tmpl.unwrap_or(DEF_REPORT_TEMPLATE.to_owned()),
            clear_tmpl: self.clear_tmpl.unwrap_or(DEF_CLEAR_TEMPLATE.to_owned()),
        };
        r.initialize()?;
        Ok(r)
    }
}

impl SlackReporter {
    fn initialize(&mut self) -> Result<(), upon::Error> {
        self.renderer
            .add_template("report", self.report_tmpl.clone())?;
        self.renderer
            .add_template("clear", self.clear_tmpl.clone())?;
        Ok(())
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
