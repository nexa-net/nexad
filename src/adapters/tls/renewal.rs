use std::sync::Arc;
use std::time::Duration;

use tracing::{error, info, warn};

use nexa_core::ports::route_store::RouteStore;

use super::acme::AcmeManager;

pub fn spawn_renewal_task(
    store: Arc<dyn RouteStore>,
    acme: Arc<AcmeManager>,
    check_interval: Duration,
    renew_before_days: i64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!(
            interval_secs = check_interval.as_secs(),
            renew_before_days, "TLS auto-renewal task started"
        );

        loop {
            tokio::time::sleep(check_interval).await;

            match store.list_expiring_certificates(renew_before_days).await {
                Ok(certs) => {
                    if certs.is_empty() {
                        info!("no certificates expiring within {renew_before_days} days");
                        continue;
                    }

                    info!(
                        count = certs.len(),
                        "found expiring certificates, attempting renewal"
                    );

                    for cert in &certs {
                        info!(domain = cert.domain, expires_at = %cert.expires_at, "renewing certificate");

                        match acme.issue_certificate(&cert.domain).await {
                            Ok(new_cert) => {
                                if let Err(e) = store.upsert_certificate(&new_cert).await {
                                    error!(domain = cert.domain, %e, "failed to store renewed certificate");
                                } else {
                                    info!(domain = cert.domain, "certificate renewed successfully");
                                }
                            }
                            Err(e) => {
                                warn!(domain = cert.domain, %e, "failed to renew certificate");
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(%e, "failed to list expiring certificates");
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::state::memory_route_store::InMemoryRouteStore;
    use nexa_core::domain::models::Certificate;

    #[tokio::test]
    async fn renewal_task_starts_and_can_be_cancelled() {
        let store = Arc::new(InMemoryRouteStore::new());
        let acme = Arc::new(AcmeManager::new("test@example.com", store.clone(), true));

        let handle = spawn_renewal_task(store, acme, Duration::from_millis(50), 30);

        tokio::time::sleep(Duration::from_millis(100)).await;
        handle.abort();
        assert!(handle.await.unwrap_err().is_cancelled());
    }

    #[tokio::test]
    async fn renewal_task_finds_expiring_certs() {
        let store = Arc::new(InMemoryRouteStore::new());

        let cert = Certificate {
            domain: "expiring.example.com".into(),
            cert_pem: b"OLD CERT".to_vec(),
            key_pem_enc: b"OLD KEY".to_vec(),
            key_nonce: b"NONCE".to_vec(),
            issued_at: chrono::Utc::now() - chrono::Duration::days(80),
            expires_at: chrono::Utc::now() + chrono::Duration::days(10),
            acme_account: None,
        };
        store.upsert_certificate(&cert).await.unwrap();

        let acme = Arc::new(AcmeManager::new("test@example.com", store.clone(), true));

        let handle = spawn_renewal_task(store.clone(), acme, Duration::from_millis(50), 30);

        tokio::time::sleep(Duration::from_millis(200)).await;
        handle.abort();

        let cert = store.get_certificate("expiring.example.com").await.unwrap();
        assert!(cert.is_some());
    }
}
