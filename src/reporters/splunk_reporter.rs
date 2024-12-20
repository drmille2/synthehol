use crate::monitor::{MonitorResult, Reporter};
use async_trait::async_trait;
use serde::Serialize;
use tracing::debug;
use tracing::error;
use tracing::instrument;

#[derive(Debug)]
pub struct SplunkReporter {
    endpoint: String,
    index: String,
    hec_token: String,
}

#[derive(Serialize, Debug)]
struct SplunkMsg {
    source: String,
    index: String,
    sourcetype: String,
    event: SplunkEvent,
}

#[derive(Serialize, Debug)]
struct SplunkEvent {
    name: String,
    level: String,
    target: String,
    arguments: String,
    stdout: String,
    stderr: String,
    duration: u64,
    status: i32,
}

impl SplunkReporter {
    pub fn from_toml(config: &toml::Table) -> Result<Self, String> {
        let endpoint = config["endpoint"]
            .as_str()
            .ok_or("missing Splunk endpoint config item")?
            .to_string();
        let index = config["index"]
            .as_str()
            .ok_or("missing Splunk index config item")?
            .to_string();
        let hec_token = config["hec_token"]
            .as_str()
            .ok_or("missing Splunk hec_token config item")?
            .to_string();
        Ok(Self {
            endpoint,
            index,
            hec_token,
        })
    }

    #[instrument]
    fn format(&self, output: &MonitorResult) -> SplunkMsg {
        let event = SplunkEvent {
            name: output.name.clone(),
            level: output.level_name.clone(),
            target: output.target.clone(),
            arguments: output.args.clone(),
            stdout: output.stdout.clone(),
            stderr: output.stderr.clone(),
            duration: output.duration,
            status: output.status,
        };
        SplunkMsg {
            source: String::from("Synthehol"),
            index: self.index.clone(),
            sourcetype: String::from("_json"),
            event,
        }
    }
}

#[async_trait]
impl Reporter for SplunkReporter {
    #[instrument]
    async fn report(&mut self, output: &MonitorResult) {
        let client = reqwest::Client::new();
        let output = self.format(output);
        let res = client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Splunk {}", self.hec_token))
            .json(&output)
            .send()
            .await;
        match res {
            Ok(r) => debug!("splunk report successful ({})", r.status()),
            Err(e) => error!("splunk report failed ({})", e),
        }
    }

    async fn clear(&mut self, _: &MonitorResult) {
        // nothing to do here for splunk
        return;
    }

    fn get_state(&self) -> Option<Vec<u8>> {
        // Vec::new()
        None
    }

    fn load_state(&mut self, _: Vec<u8>) {}
}
