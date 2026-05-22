use std::collections::BTreeMap;
use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;

use nexa_core::error::{NexaError, Result};
use nexa_core::ports::proxy::{ProxyBackend, RouteConfig, TlsConfig};

// --- Traefik dynamic configuration YAML structs ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TraefikDynamicConfig {
    pub http: TraefikHttp,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TraefikHttp {
    pub routers: BTreeMap<String, TraefikRouter>,
    pub services: BTreeMap<String, TraefikService>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraefikRouter {
    pub rule: String,
    #[serde(rename = "entryPoints")]
    pub entry_points: Vec<String>,
    pub service: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<TraefikTls>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraefikTls {
    #[serde(rename = "certResolver", skip_serializing_if = "Option::is_none")]
    pub cert_resolver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraefikService {
    #[serde(rename = "loadBalancer")]
    pub load_balancer: TraefikLoadBalancer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraefikLoadBalancer {
    pub servers: Vec<TraefikServer>,
    #[serde(rename = "healthCheck")]
    pub health_check: TraefikHealthCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraefikServer {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraefikHealthCheck {
    pub path: String,
    pub interval: String,
}

// --- TraefikBackend ---

pub struct TraefikBackend {
    config_path: PathBuf,
}

impl TraefikBackend {
    pub fn new(config_path: PathBuf) -> Self {
        Self { config_path }
    }

    fn render_config(routes: &[RouteConfig]) -> TraefikDynamicConfig {
        let mut config = TraefikDynamicConfig::default();

        for route in routes {
            let sanitized = route.domain.replace('.', "-");
            let router_name = format!("nexa-{sanitized}");
            let service_name = format!("nexa-svc-{sanitized}");

            let (entry_points, tls) = match &route.tls {
                TlsConfig::None => (vec!["web".to_string()], None),
                TlsConfig::Auto { .. } => (
                    vec!["websecure".to_string()],
                    Some(TraefikTls {
                        cert_resolver: Some("letsencrypt".to_string()),
                    }),
                ),
                TlsConfig::Manual { .. } => (
                    vec!["websecure".to_string()],
                    Some(TraefikTls {
                        cert_resolver: None,
                    }),
                ),
            };

            let router = TraefikRouter {
                rule: format!("Host(`{}`)", route.domain),
                entry_points,
                service: service_name.clone(),
                tls,
            };

            let servers: Vec<TraefikServer> = route
                .upstream
                .iter()
                .map(|u| TraefikServer {
                    url: format!("http://{}", u.address),
                    weight: if u.weight > 1 { Some(u.weight) } else { None },
                })
                .collect();

            let service = TraefikService {
                load_balancer: TraefikLoadBalancer {
                    servers,
                    health_check: TraefikHealthCheck {
                        path: "/health".to_string(),
                        interval: "10s".to_string(),
                    },
                },
            };

            config.http.routers.insert(router_name, router);
            config.http.services.insert(service_name, service);
        }

        config
    }
}

#[async_trait]
impl ProxyBackend for TraefikBackend {
    async fn apply_routes(&self, routes: &[RouteConfig]) -> Result<()> {
        let config = Self::render_config(routes);
        let yaml = serde_yaml::to_string(&config)
            .map_err(|e| NexaError::Proxy(format!("failed to serialize traefik config: {e}")))?;

        tokio::fs::write(&self.config_path, &yaml)
            .await
            .map_err(|e| {
                NexaError::Proxy(format!(
                    "failed to write traefik config {}: {e}",
                    self.config_path.display()
                ))
            })?;
        info!(path = %self.config_path.display(), "wrote traefik dynamic config");
        Ok(())
    }

    async fn remove_route(&self, domain: &str) -> Result<()> {
        let content = match tokio::fs::read_to_string(&self.config_path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(e) => {
                return Err(NexaError::Proxy(format!(
                    "failed to read traefik config: {e}"
                )));
            }
        };

        let mut config: TraefikDynamicConfig = serde_yaml::from_str(&content)
            .map_err(|e| NexaError::Proxy(format!("failed to parse traefik config: {e}")))?;

        let sanitized = domain.replace('.', "-");
        let router_name = format!("nexa-{sanitized}");
        let service_name = format!("nexa-svc-{sanitized}");

        config.http.routers.remove(&router_name);
        config.http.services.remove(&service_name);

        let yaml = serde_yaml::to_string(&config)
            .map_err(|e| NexaError::Proxy(format!("failed to serialize traefik config: {e}")))?;

        tokio::fs::write(&self.config_path, &yaml)
            .await
            .map_err(|e| NexaError::Proxy(format!("failed to rewrite traefik config: {e}")))?;
        info!(domain, "removed route from traefik config");
        Ok(())
    }

    async fn reload(&self) -> Result<()> {
        // Traefik watches config files for changes automatically; no action needed.
        info!("traefik reload is a no-op (file watch mode)");
        Ok(())
    }

    async fn health(&self) -> Result<bool> {
        let content = match tokio::fs::read_to_string(&self.config_path).await {
            Ok(c) => c,
            Err(_) => return Ok(false),
        };

        match serde_yaml::from_str::<TraefikDynamicConfig>(&content) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexa_core::ports::proxy::Upstream;

    fn sample_routes() -> Vec<RouteConfig> {
        vec![
            RouteConfig {
                domain: "app.example.com".into(),
                upstream: vec![
                    Upstream {
                        address: "10.0.0.1:8080".into(),
                        weight: 3,
                    },
                    Upstream {
                        address: "10.0.0.2:8080".into(),
                        weight: 1,
                    },
                ],
                tls: TlsConfig::Auto {
                    email: "admin@example.com".into(),
                },
            },
            RouteConfig {
                domain: "plain.example.com".into(),
                upstream: vec![Upstream {
                    address: "10.0.0.5:3000".into(),
                    weight: 1,
                }],
                tls: TlsConfig::None,
            },
        ]
    }

    #[test]
    fn render_config_produces_valid_yaml() {
        let config = TraefikBackend::render_config(&sample_routes());
        let yaml = serde_yaml::to_string(&config).unwrap();

        // Verify it roundtrips
        let parsed: TraefikDynamicConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.http.routers.len(), 2);
        assert_eq!(parsed.http.services.len(), 2);

        // Check router naming
        assert!(parsed.http.routers.contains_key("nexa-app-example-com"));
        assert!(parsed.http.routers.contains_key("nexa-plain-example-com"));

        // Check service naming
        assert!(
            parsed
                .http
                .services
                .contains_key("nexa-svc-app-example-com")
        );
        assert!(
            parsed
                .http
                .services
                .contains_key("nexa-svc-plain-example-com")
        );
    }

    #[test]
    fn render_config_no_tls() {
        let routes = vec![RouteConfig {
            domain: "plain.example.com".into(),
            upstream: vec![Upstream {
                address: "10.0.0.1:3000".into(),
                weight: 1,
            }],
            tls: TlsConfig::None,
        }];
        let config = TraefikBackend::render_config(&routes);
        let router = &config.http.routers["nexa-plain-example-com"];

        assert_eq!(router.entry_points, vec!["web"]);
        assert!(router.tls.is_none());
        assert_eq!(router.rule, "Host(`plain.example.com`)");
    }

    #[test]
    fn render_config_health_check() {
        let config = TraefikBackend::render_config(&sample_routes());
        let svc = &config.http.services["nexa-svc-app-example-com"];

        assert_eq!(svc.load_balancer.health_check.path, "/health");
        assert_eq!(svc.load_balancer.health_check.interval, "10s");
    }

    #[tokio::test]
    async fn apply_routes_writes_yaml_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("traefik-dynamic.yml");
        let backend = TraefikBackend::new(config_path.clone());

        backend.apply_routes(&sample_routes()).await.unwrap();

        assert!(config_path.exists());
        let content = tokio::fs::read_to_string(&config_path).await.unwrap();

        // Parse back and verify
        let parsed: TraefikDynamicConfig = serde_yaml::from_str(&content).unwrap();
        assert_eq!(parsed.http.routers.len(), 2);
    }

    #[tokio::test]
    async fn remove_route_from_traefik_config() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("traefik-dynamic.yml");
        let backend = TraefikBackend::new(config_path.clone());

        backend.apply_routes(&sample_routes()).await.unwrap();

        // Remove one route
        backend.remove_route("app.example.com").await.unwrap();

        let content = tokio::fs::read_to_string(&config_path).await.unwrap();
        let parsed: TraefikDynamicConfig = serde_yaml::from_str(&content).unwrap();

        assert_eq!(parsed.http.routers.len(), 1);
        assert!(!parsed.http.routers.contains_key("nexa-app-example-com"));
        assert!(parsed.http.routers.contains_key("nexa-plain-example-com"));
        assert_eq!(parsed.http.services.len(), 1);
    }

    #[tokio::test]
    async fn health_returns_true_for_valid_config() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("traefik-dynamic.yml");
        let backend = TraefikBackend::new(config_path.clone());

        backend.apply_routes(&sample_routes()).await.unwrap();

        let healthy = backend.health().await.unwrap();
        assert!(healthy);
    }

    #[tokio::test]
    async fn health_returns_false_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("nonexistent.yml");
        let backend = TraefikBackend::new(config_path);

        let healthy = backend.health().await.unwrap();
        assert!(!healthy);
    }

    #[tokio::test]
    async fn reload_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("traefik-dynamic.yml");
        let backend = TraefikBackend::new(config_path);

        // reload should always succeed, even without a config file
        let result = backend.reload().await;
        assert!(result.is_ok());
    }
}
