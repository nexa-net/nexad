use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;

use async_trait::async_trait;
use tokio::process::{Child, Command};
use tracing::{info, warn};

use nexa_core::error::{NexaError, Result};
use nexa_core::ports::proxy::{ProxyBackend, RouteConfig, TlsConfig};

pub struct NexaProxyBackend {
    config_path: PathBuf,
    binary_path: String,
    http_listen: String,
    https_listen: Option<String>,
    child: Mutex<Option<Child>>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct NexaProxyConfig {
    http_listen: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    https_listen: Option<String>,
    routes: HashMap<String, NexaProxyRoute>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct NexaProxyRoute {
    upstreams: Vec<NexaProxyUpstream>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tls: Option<NexaProxyTls>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct NexaProxyUpstream {
    address: String,
    weight: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct NexaProxyTls {
    #[serde(skip_serializing_if = "Option::is_none")]
    cert_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    acme_email: Option<String>,
}

impl NexaProxyBackend {
    pub fn new(
        config_path: impl Into<PathBuf>,
        binary_path: impl Into<String>,
        http_listen: impl Into<String>,
        https_listen: Option<String>,
    ) -> Self {
        Self {
            config_path: config_path.into(),
            binary_path: binary_path.into(),
            http_listen: http_listen.into(),
            https_listen,
            child: Mutex::new(None),
        }
    }

    fn build_config(&self, routes: &[RouteConfig]) -> NexaProxyConfig {
        let mut route_map = HashMap::new();

        for route in routes {
            let upstreams: Vec<NexaProxyUpstream> = route
                .upstream
                .iter()
                .map(|u| NexaProxyUpstream {
                    address: u.address.clone(),
                    weight: u.weight,
                })
                .collect();

            let tls = match &route.tls {
                TlsConfig::None => None,
                TlsConfig::Auto { email } => Some(NexaProxyTls {
                    cert_path: None,
                    key_path: None,
                    acme_email: Some(email.clone()),
                }),
                TlsConfig::Manual { cert, key } => Some(NexaProxyTls {
                    cert_path: Some(cert.clone()),
                    key_path: Some(key.clone()),
                    acme_email: None,
                }),
            };

            route_map.insert(route.domain.clone(), NexaProxyRoute { upstreams, tls });
        }

        NexaProxyConfig {
            http_listen: self.http_listen.clone(),
            https_listen: self.https_listen.clone(),
            routes: route_map,
        }
    }

    fn write_config_sync(&self, config: &NexaProxyConfig) -> Result<()> {
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| NexaError::Proxy(format!("failed to serialize nexa-proxy config: {e}")))?;
        std::fs::write(&self.config_path, &json).map_err(|e| {
            NexaError::Proxy(format!(
                "failed to write nexa-proxy config {}: {e}",
                self.config_path.display()
            ))
        })?;
        Ok(())
    }

    fn start_child(&self) -> Result<()> {
        let mut child_lock = self.child.lock().unwrap();

        if let Some(ref mut child) = *child_lock {
            let _ = child.start_kill();
        }

        let child = Command::new(&self.binary_path)
            .arg("--config")
            .arg(&self.config_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                NexaError::Proxy(format!(
                    "failed to spawn nexa-proxy binary '{}': {e}",
                    self.binary_path
                ))
            })?;

        info!(pid = child.id().unwrap_or(0), "nexa-proxy child started");
        *child_lock = Some(child);
        Ok(())
    }
}

#[async_trait]
impl ProxyBackend for NexaProxyBackend {
    async fn apply_routes(&self, routes: &[RouteConfig]) -> Result<()> {
        let config = self.build_config(routes);
        self.write_config_sync(&config)?;
        info!(path = %self.config_path.display(), routes = routes.len(), "nexa-proxy config written");
        Ok(())
    }

    async fn remove_route(&self, domain: &str) -> Result<()> {
        let content = match std::fs::read_to_string(&self.config_path) {
            Ok(c) => c,
            Err(_) => {
                warn!(domain, "nexa-proxy config not found");
                return Ok(());
            }
        };

        let mut config: NexaProxyConfig = serde_json::from_str(&content)
            .map_err(|e| NexaError::Proxy(format!("failed to parse nexa-proxy config: {e}")))?;

        config.routes.remove(domain);
        self.write_config_sync(&config)?;
        info!(domain, "route removed from nexa-proxy config");
        Ok(())
    }

    async fn reload(&self) -> Result<()> {
        self.start_child()?;
        Ok(())
    }

    async fn health(&self) -> Result<bool> {
        let child_lock = self.child.lock().unwrap();
        match &*child_lock {
            Some(child) => Ok(child.id().is_some()),
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexa_core::ports::proxy::Upstream;
    use std::path::PathBuf;

    fn make_routes() -> Vec<RouteConfig> {
        vec![
            RouteConfig {
                domain: "api.example.com".into(),
                upstream: vec![
                    Upstream {
                        address: "10.0.0.1:3000".into(),
                        weight: 1,
                    },
                    Upstream {
                        address: "10.0.0.2:3000".into(),
                        weight: 2,
                    },
                ],
                tls: TlsConfig::Auto {
                    email: "admin@example.com".into(),
                },
            },
            RouteConfig {
                domain: "static.example.com".into(),
                upstream: vec![Upstream {
                    address: "10.0.0.5:80".into(),
                    weight: 1,
                }],
                tls: TlsConfig::None,
            },
        ]
    }

    #[test]
    fn build_config_json() {
        let backend = NexaProxyBackend::new(
            "/tmp/test-nexa-proxy.json",
            "nexa-proxy",
            "0.0.0.0:80",
            Some("0.0.0.0:443".into()),
        );
        let config = backend.build_config(&make_routes());
        assert_eq!(config.routes.len(), 2);
        assert!(config.routes.contains_key("api.example.com"));
        assert!(config.routes.contains_key("static.example.com"));

        let api = &config.routes["api.example.com"];
        assert_eq!(api.upstreams.len(), 2);
        assert_eq!(api.upstreams[1].weight, 2);
        assert_eq!(
            api.tls.as_ref().unwrap().acme_email.as_deref(),
            Some("admin@example.com")
        );

        let st = &config.routes["static.example.com"];
        assert!(st.tls.is_none());
    }

    #[test]
    fn build_config_manual_tls() {
        let routes = vec![RouteConfig {
            domain: "manual.example.com".into(),
            upstream: vec![Upstream {
                address: "10.0.0.1:443".into(),
                weight: 1,
            }],
            tls: TlsConfig::Manual {
                cert: PathBuf::from("/certs/cert.pem"),
                key: PathBuf::from("/certs/key.pem"),
            },
        }];
        let backend = NexaProxyBackend::new("/tmp/test.json", "nexa-proxy", "0.0.0.0:80", None);
        let config = backend.build_config(&routes);
        let tls = config.routes["manual.example.com"]
            .tls
            .as_ref()
            .unwrap();
        assert_eq!(
            tls.cert_path.as_ref().unwrap(),
            &PathBuf::from("/certs/cert.pem")
        );
        assert!(tls.acme_email.is_none());
    }

    #[tokio::test]
    async fn apply_routes_writes_json_file() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("proxy.json");
        let backend = NexaProxyBackend::new(&config_path, "nexa-proxy", "0.0.0.0:80", None);

        backend.apply_routes(&make_routes()).await.unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["routes"]["api.example.com"].is_object());
        assert!(parsed["routes"]["static.example.com"].is_object());
    }

    #[tokio::test]
    async fn remove_route_from_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("proxy.json");
        let backend = NexaProxyBackend::new(&config_path, "nexa-proxy", "0.0.0.0:80", None);

        backend.apply_routes(&make_routes()).await.unwrap();
        backend.remove_route("api.example.com").await.unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["routes"]["api.example.com"].is_null());
        assert!(parsed["routes"]["static.example.com"].is_object());
    }

    #[tokio::test]
    async fn health_returns_false_when_no_child() {
        let backend =
            NexaProxyBackend::new("/tmp/nope.json", "nexa-proxy", "0.0.0.0:80", None);
        assert!(!backend.health().await.unwrap());
    }
}
