use std::sync::Arc;

use tracing::info;

use nexa_core::domain::models::Certificate;
use nexa_core::error::{NexaError, Result};
use nexa_core::ports::route_store::RouteStore;

pub struct AcmeManager {
    email: String,
    store: Arc<dyn RouteStore>,
    staging: bool,
}

impl AcmeManager {
    pub fn new(email: &str, store: Arc<dyn RouteStore>, staging: bool) -> Self {
        Self {
            email: email.to_string(),
            store,
            staging,
        }
    }

    pub async fn issue_certificate(&self, domain: &str) -> Result<Certificate> {
        info!(
            domain,
            email = self.email,
            staging = self.staging,
            "initiating ACME certificate issuance"
        );
        Err(NexaError::Certificate(format!(
            "ACME issuance for '{domain}' requires network access and HTTP challenge validation"
        )))
    }

    pub async fn import_certificate(
        &self,
        domain: &str,
        cert_pem: Vec<u8>,
        key_pem: Vec<u8>,
    ) -> Result<()> {
        let cert = Certificate {
            domain: domain.to_string(),
            cert_pem,
            key_pem_enc: key_pem,
            key_nonce: vec![0u8; 12],
            issued_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::days(90),
            acme_account: None,
        };
        self.store.upsert_certificate(&cert).await?;
        info!(domain, "certificate imported");
        Ok(())
    }

    pub fn email(&self) -> &str {
        &self.email
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::state::memory_route_store::InMemoryRouteStore;

    fn make_acme() -> AcmeManager {
        let store = Arc::new(InMemoryRouteStore::new());
        AcmeManager::new("admin@example.com", store, true)
    }

    #[test]
    fn acme_manager_email() {
        let acme = make_acme();
        assert_eq!(acme.email(), "admin@example.com");
    }

    #[tokio::test]
    async fn issue_certificate_returns_error_placeholder() {
        let acme = make_acme();
        let result = acme.issue_certificate("api.example.com").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("ACME issuance"));
    }

    #[tokio::test]
    async fn import_certificate_stores_in_route_store() {
        let store = Arc::new(InMemoryRouteStore::new());
        let acme = AcmeManager::new("admin@example.com", store.clone(), true);

        acme.import_certificate(
            "api.example.com",
            b"CERT PEM DATA".to_vec(),
            b"KEY PEM DATA".to_vec(),
        )
        .await
        .unwrap();

        let cert = store
            .get_certificate("api.example.com")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cert.cert_pem, b"CERT PEM DATA");
        assert_eq!(cert.key_pem_enc, b"KEY PEM DATA");
    }
}
