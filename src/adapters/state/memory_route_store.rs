use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use chrono::{Duration, Utc};

use nexa_core::domain::models::{Certificate, Route, SubnetAllocation};
use nexa_core::error::{NexaError, Result};
use nexa_core::ports::route_store::RouteStore;

pub struct InMemoryRouteStore {
    routes: RwLock<HashMap<String, Route>>,
    certificates: RwLock<HashMap<String, Certificate>>,
    subnets: RwLock<Vec<SubnetAllocation>>,
}

impl InMemoryRouteStore {
    pub fn new() -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
            certificates: RwLock::new(HashMap::new()),
            subnets: RwLock::new(Vec::new()),
        }
    }
}

#[async_trait]
impl RouteStore for InMemoryRouteStore {
    async fn insert_route(&self, route: &Route) -> Result<()> {
        let mut routes = self.routes.write().unwrap();
        if routes.contains_key(&route.domain) {
            return Err(NexaError::RouteAlreadyExists(route.domain.clone()));
        }
        routes.insert(route.domain.clone(), route.clone());
        Ok(())
    }

    async fn get_route(&self, domain: &str) -> Result<Option<Route>> {
        let routes = self.routes.read().unwrap();
        Ok(routes.get(domain).cloned())
    }

    async fn list_routes(&self, project: Option<&str>) -> Result<Vec<Route>> {
        let routes = self.routes.read().unwrap();
        let result: Vec<Route> = routes
            .values()
            .filter(|r| match project {
                Some(p) => r.project == p,
                None => true,
            })
            .cloned()
            .collect();
        Ok(result)
    }

    async fn delete_route(&self, domain: &str) -> Result<bool> {
        let mut routes = self.routes.write().unwrap();
        Ok(routes.remove(domain).is_some())
    }

    async fn upsert_certificate(&self, cert: &Certificate) -> Result<()> {
        let mut certs = self.certificates.write().unwrap();
        certs.insert(cert.domain.clone(), cert.clone());
        Ok(())
    }

    async fn get_certificate(&self, domain: &str) -> Result<Option<Certificate>> {
        let certs = self.certificates.read().unwrap();
        Ok(certs.get(domain).cloned())
    }

    async fn list_expiring_certificates(&self, within_days: i64) -> Result<Vec<Certificate>> {
        let certs = self.certificates.read().unwrap();
        let threshold = Utc::now() + Duration::days(within_days);
        let result: Vec<Certificate> = certs
            .values()
            .filter(|c| c.expires_at <= threshold)
            .cloned()
            .collect();
        Ok(result)
    }

    async fn delete_certificate(&self, domain: &str) -> Result<bool> {
        let mut certs = self.certificates.write().unwrap();
        Ok(certs.remove(domain).is_some())
    }

    async fn allocate_subnet(&self, alloc: &SubnetAllocation) -> Result<()> {
        let mut subnets = self.subnets.write().unwrap();
        let exists = subnets
            .iter()
            .any(|s| s.node_id == alloc.node_id && s.project == alloc.project);
        if exists {
            return Err(NexaError::Network(format!(
                "subnet already allocated for node {} project {}",
                alloc.node_id, alloc.project
            )));
        }
        let subnet_taken = subnets.iter().any(|s| s.subnet == alloc.subnet);
        if subnet_taken {
            return Err(NexaError::Network(format!(
                "subnet {} already in use",
                alloc.subnet
            )));
        }
        subnets.push(alloc.clone());
        Ok(())
    }

    async fn get_node_subnet(
        &self,
        node_id: &str,
        project: &str,
    ) -> Result<Option<SubnetAllocation>> {
        let subnets = self.subnets.read().unwrap();
        Ok(subnets
            .iter()
            .find(|s| s.node_id == node_id && s.project == project)
            .cloned())
    }

    async fn list_subnets(&self) -> Result<Vec<SubnetAllocation>> {
        let subnets = self.subnets.read().unwrap();
        Ok(subnets.clone())
    }

    async fn deallocate_subnet(&self, node_id: &str, project: &str) -> Result<bool> {
        let mut subnets = self.subnets.write().unwrap();
        let len_before = subnets.len();
        subnets.retain(|s| !(s.node_id == node_id && s.project == project));
        Ok(subnets.len() < len_before)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexa_core::domain::models::TlsMode;

    #[tokio::test]
    async fn insert_and_get_route() {
        let store = InMemoryRouteStore::new();
        let route = Route::new("api.example.com", "ecommerce", "api", TlsMode::Auto);
        store.insert_route(&route).await.unwrap();

        let fetched = store.get_route("api.example.com").await.unwrap().unwrap();
        assert_eq!(fetched.domain, "api.example.com");
        assert_eq!(fetched.project, "ecommerce");
        assert_eq!(fetched.tls_mode, TlsMode::Auto);
    }

    #[tokio::test]
    async fn insert_duplicate_route_fails() {
        let store = InMemoryRouteStore::new();
        let route = Route::new("api.example.com", "ecommerce", "api", TlsMode::None);
        store.insert_route(&route).await.unwrap();
        assert!(store.insert_route(&route).await.is_err());
    }

    #[tokio::test]
    async fn list_routes_filter_by_project() {
        let store = InMemoryRouteStore::new();
        store
            .insert_route(&Route::new("a.example.com", "proj-a", "api", TlsMode::None))
            .await
            .unwrap();
        store
            .insert_route(&Route::new("b.example.com", "proj-b", "web", TlsMode::Auto))
            .await
            .unwrap();

        let all = store.list_routes(None).await.unwrap();
        assert_eq!(all.len(), 2);

        let proj_a = store.list_routes(Some("proj-a")).await.unwrap();
        assert_eq!(proj_a.len(), 1);
        assert_eq!(proj_a[0].domain, "a.example.com");
    }

    #[tokio::test]
    async fn delete_route() {
        let store = InMemoryRouteStore::new();
        store
            .insert_route(&Route::new("api.example.com", "p", "d", TlsMode::None))
            .await
            .unwrap();
        assert!(store.delete_route("api.example.com").await.unwrap());
        assert!(!store.delete_route("api.example.com").await.unwrap());
        assert!(store.get_route("api.example.com").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn upsert_and_get_certificate() {
        let store = InMemoryRouteStore::new();
        let cert = Certificate {
            domain: "api.example.com".into(),
            cert_pem: b"CERT".to_vec(),
            key_pem_enc: b"KEY".to_vec(),
            key_nonce: b"NONCE".to_vec(),
            issued_at: Utc::now(),
            expires_at: Utc::now() + Duration::days(90),
            acme_account: None,
        };
        store.upsert_certificate(&cert).await.unwrap();

        let fetched = store
            .get_certificate("api.example.com")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.cert_pem, b"CERT");
    }

    #[tokio::test]
    async fn list_expiring_certificates() {
        let store = InMemoryRouteStore::new();
        let expiring_soon = Certificate {
            domain: "soon.example.com".into(),
            cert_pem: b"C".to_vec(),
            key_pem_enc: b"K".to_vec(),
            key_nonce: b"N".to_vec(),
            issued_at: Utc::now() - Duration::days(60),
            expires_at: Utc::now() + Duration::days(20),
            acme_account: None,
        };
        let not_expiring = Certificate {
            domain: "ok.example.com".into(),
            cert_pem: b"C".to_vec(),
            key_pem_enc: b"K".to_vec(),
            key_nonce: b"N".to_vec(),
            issued_at: Utc::now(),
            expires_at: Utc::now() + Duration::days(80),
            acme_account: None,
        };
        store.upsert_certificate(&expiring_soon).await.unwrap();
        store.upsert_certificate(&not_expiring).await.unwrap();

        let expiring = store.list_expiring_certificates(30).await.unwrap();
        assert_eq!(expiring.len(), 1);
        assert_eq!(expiring[0].domain, "soon.example.com");
    }

    #[tokio::test]
    async fn allocate_and_list_subnets() {
        let store = InMemoryRouteStore::new();
        let alloc = SubnetAllocation {
            node_id: "node-1".into(),
            project: "ecommerce".into(),
            subnet: "172.20.1.0/24".into(),
        };
        store.allocate_subnet(&alloc).await.unwrap();

        let subnets = store.list_subnets().await.unwrap();
        assert_eq!(subnets.len(), 1);
        assert_eq!(subnets[0].subnet, "172.20.1.0/24");
    }

    #[tokio::test]
    async fn allocate_duplicate_subnet_fails() {
        let store = InMemoryRouteStore::new();
        let alloc = SubnetAllocation {
            node_id: "node-1".into(),
            project: "ecommerce".into(),
            subnet: "172.20.1.0/24".into(),
        };
        store.allocate_subnet(&alloc).await.unwrap();
        assert!(store.allocate_subnet(&alloc).await.is_err());
    }

    #[tokio::test]
    async fn allocate_same_subnet_different_node_fails() {
        let store = InMemoryRouteStore::new();
        store
            .allocate_subnet(&SubnetAllocation {
                node_id: "node-1".into(),
                project: "ecommerce".into(),
                subnet: "172.20.1.0/24".into(),
            })
            .await
            .unwrap();
        let result = store
            .allocate_subnet(&SubnetAllocation {
                node_id: "node-2".into(),
                project: "ecommerce".into(),
                subnet: "172.20.1.0/24".into(),
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn deallocate_subnet() {
        let store = InMemoryRouteStore::new();
        store
            .allocate_subnet(&SubnetAllocation {
                node_id: "node-1".into(),
                project: "p".into(),
                subnet: "172.20.1.0/24".into(),
            })
            .await
            .unwrap();
        assert!(store.deallocate_subnet("node-1", "p").await.unwrap());
        assert!(!store.deallocate_subnet("node-1", "p").await.unwrap());
    }
}
