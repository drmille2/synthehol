use crate::monitor::{MonitorResult, Reporter};
use async_trait::async_trait;
use serde::Serialize;
use tracing::debug;
use tracing::error;
use tracing::instrument;

#[derive(Debug)]
pub struct PagerdutyReporter {
    appid: String,
    api_key: String,
    endpoint: String
}

#[derive(Serialize, Debug)]
struct PagerdutyEvent { }

#[derive(Serialize, Debug)]
struct PagerdutyMsg { }

impl PagerdutyReporter {
    pub fn from_toml(config: &toml::Table) -> Result<Self, String> {
        let appid = config["appid"]
            .as_str()
            .ok_or("missing Pagerduty appid config item")?
            .to_string();
        let api_key = config["api_key"]
            .as_str()
            .ok_or("missing Pagerduty api_key config item")?
            .to_string();
        let endpoint = config["endpoint"]
            .as_str()
            .ok_or("missing Pagerduty endpoint config item")?
            .to_string();
        Ok(Self{
            appid,
            api_key,
            endpoint
        })
    }

    #[instrument]
    fn format(&self, _output: &MonitorResult) -> PagerdutyMsg {
        PagerdutyMsg{}
    }
}

#[async_trait]
impl Reporter for PagerdutyReporter {
    #[instrument]
    async fn report(&self, output: &MonitorResult) {
        let client = reqwest::Client::new();
        let output = self.format(output);
        let res = client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .json(&output)
            .send()
            .await;
        match res {
            Ok(r) => debug!("pagerduty report successful ({})", r.status()),
            Err(e) => error!("pagerduty report failed ({})", e),
        }
    }

    #[instrument]
    async fn clear(&self, _: &MonitorResult) { }
}