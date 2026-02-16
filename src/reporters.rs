pub mod pagerduty;
pub mod postgresql;
pub mod slack;
pub mod splunk;

use async_trait::async_trait;

use crate::monitor::MonitorResult;

/// Reporters have an async report() function that handles a monitor result
/// taking care of any formatting and delivery required
#[async_trait]
pub trait Reporter {
    async fn report(&mut self, _: &MonitorResult);
    async fn clear(&mut self, _: &MonitorResult);
    fn get_state(&self) -> Option<Vec<u8>>;
    fn load_state(&mut self, _: Vec<u8>);
}
