use std::sync::Arc;
use std::time::Duration;

use nexa_core::domain::orchestrator::OrchestratorHandle;
use reqwest::Client;
use tracing::{debug, info};

pub struct HealthChecker {
    handle: OrchestratorHandle,
    http_client: Client,
}

impl HealthChecker {
    pub fn new(handle: OrchestratorHandle) -> Self {
        Self {
            handle,
            http_client: Client::builder()
                .no_proxy()
                .build()
                .expect("failed to build reqwest client"),
        }
    }

    pub async fn run(self: Arc<Self>) {
        info!("health checker started");
        let mut tick = tokio::time::interval(Duration::from_secs(1));

        loop {
            tick.tick().await;

            let targets = self.handle.get_health_probe_targets().await;
            if targets.is_empty() {
                continue;
            }

            debug!(count = targets.len(), "probing pods");

            for (pod_id, config) in targets {
                let client = self.http_client.clone();
                let handle = self.handle.clone();

                tokio::spawn(async move {
                    let url = format!(
                        "http://{}:{}{}",
                        config.container_ip, config.port, config.path
                    );

                    let healthy = match tokio::time::timeout(
                        config.timeout,
                        client.get(&url).send(),
                    )
                    .await
                    {
                        Ok(Ok(response)) => response.status().is_success(),
                        Ok(Err(e)) => {
                            debug!(pod_id = %pod_id, url = %url, error = %e, "health probe failed");
                            false
                        }
                        Err(_) => {
                            debug!(pod_id = %pod_id, url = %url, "health probe timed out");
                            false
                        }
                    };

                    handle.report_health(pod_id, healthy).await;
                });
            }
        }
    }
}
