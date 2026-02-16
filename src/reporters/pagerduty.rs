use crate::monitor::MonitorResult;
use crate::reporters::Reporter;
use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::instrument;
use tracing::warn;

#[derive(Debug)]
pub struct PagerdutyReporter {
    renderer: upon::Engine<'static>,
    endpoint: String,
    routing_key: String,
    dedup_key: Option<String>,
    source: Option<String>,
    component: Option<String>,
    client: Option<String>,
    group: Option<String>,
    class: Option<String>,
    report_tmpl: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct PagerdutyReporterArgs {
    routing_key: String,
    endpoint: String,
    group: Option<String>,
    class: Option<String>,
    component: Option<String>,
    client: Option<String>,
    source: Option<String>,
    report_tmpl: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum Severity {
    // Critical,
    // Warning,
    Error,
    // Info,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Trigger,
    // Acknowledge,
    Resolve,
}

#[derive(Serialize, Debug)]
struct PagerdutyPayload {
    summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
    severity: Severity,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    component: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    class: Option<String>,
}

#[derive(Serialize, Debug)]
struct PagerdutyMsg {
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<PagerdutyPayload>,
    routing_key: String,
    event_action: Action,
    #[serde(skip_serializing_if = "Option::is_none")]
    dedup_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client: Option<String>,
}

#[derive(Deserialize, Debug)]
struct PagerdutyResponse {
    dedup_key: Option<String>,
}

impl PagerdutyReporterArgs {
    pub fn build(self) -> Result<PagerdutyReporter, upon::Error> {
        let mut r = PagerdutyReporter {
            renderer: upon::Engine::new(),
            endpoint: self.endpoint,
            routing_key: self.routing_key,
            dedup_key: None,
            source: self.source,
            component: self.component,
            client: self.client,
            group: self.group,
            class: self.class,
            report_tmpl: self.report_tmpl.unwrap_or(DEF_REPORT_TEMPLATE.to_owned()),
        };
        r.initialize()?;
        Ok(r)
    }
}

impl PagerdutyReporter {
    fn initialize(&mut self) -> Result<(), upon::Error> {
        self.renderer
            .add_template("report", self.report_tmpl.clone())
    }

    fn format(&self, output: &MonitorResult) -> Result<PagerdutyMsg, String> {
        let summary = self
            .renderer
            .template("report")
            .render(upon::value![res: output])
            .to_string()
            .map_err(|e| format!("error rendering pagerduty template {e}"))?;
        let payload = Some(PagerdutyPayload {
            summary,
            timestamp: None,
            severity: Severity::Error,
            source: self.source.clone(),
            component: self.component.clone(),
            group: self.group.clone(),
            class: self.class.clone(),
        });
        Ok(PagerdutyMsg {
            payload,
            routing_key: self.routing_key.clone(),
            event_action: Action::Trigger,
            dedup_key: self.dedup_key.clone(),
            client: self.client.clone(),
        })
    }

    async fn send(&self, content: &PagerdutyMsg) -> Result<PagerdutyResponse, String> {
        let client = reqwest::Client::new();
        let res = client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .json(content)
            .send()
            .await
            .map_err(|e| format!("failed to send pagerduty event ({})", e))?;
        let status = res.status();
        let body = res
            .text()
            .await
            .map_err(|e| format!("failed to read pagerduty response ({})", e))?;
        if status.as_u16() != 202 {
            return Err(format!(
                "non-successful pagerduty response received ({})",
                body
            ));
        }
        let v: PagerdutyResponse = serde_json::from_str(&body)
            .map_err(|e| format!("failed to deserialize pagerduty response ({})", e))?;
        Ok(v)
    }
}

#[async_trait]
impl Reporter for PagerdutyReporter {
    #[instrument]
    async fn report(&mut self, output: &MonitorResult) {
        let output = self.format(output);
        match output {
            Ok(mut output) => {
                output.event_action = Action::Trigger;
                match self.send(&output).await {
                    Ok(r) => {
                        debug!("pagerduty incident reported successfully");
                        self.dedup_key = r.dedup_key;
                    }
                    Err(e) => error!(e),
                }
            }
            Err(e) => error!(e),
        }
    }

    #[instrument]
    async fn clear(&mut self, output: &MonitorResult) {
        let output = self.format(output);
        match output {
            Ok(mut output) => {
                output.event_action = Action::Resolve;
                match self.send(&output).await {
                    Ok(_) => {
                        debug!("pagerduty incident cleared successfully");
                        self.dedup_key = None;
                    }
                    Err(e) => error!(e),
                }
            }
            Err(e) => warn!(e),
        }
    }

    fn get_state(&self) -> Option<Vec<u8>> {
        self.dedup_key.clone().map(|x| x.into_bytes())
    }

    fn load_state(&mut self, state: Vec<u8>) {
        match String::from_utf8(state) {
            Ok(state) => {
                info!("loaded pagerduty state ({})", state);
                self.dedup_key = Some(state);
            }
            Err(e) => {
                error!("failed to load pagerduty state ({})", e);
                self.dedup_key = None
            }
        }
    }
}

const DEF_REPORT_TEMPLATE: &str = "Monitor: {{res.name}} triggered [level: {{res.level_name}}] 
command: {{ res.target }} 
args: {{ res.args }} 
result: {{ res.status }} 
duration: {{ res.duration }} μs";
