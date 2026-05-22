use std::path::PathBuf;

use async_trait::async_trait;
use tracing::{info, warn};

use nexa_core::error::{NexaError, Result};
use nexa_core::ports::proxy::{ProxyBackend, RouteConfig, TlsConfig};

pub struct CaddyBackend {
    caddyfile_path: PathBuf,
    admin_api: String,
}

impl CaddyBackend {
    pub fn new(caddyfile_path: PathBuf, admin_api: String) -> Self {
        Self {
            caddyfile_path,
            admin_api,
        }
    }

    fn render_caddyfile(routes: &[RouteConfig]) -> String {
        let mut output = String::new();

        for (i, route) in routes.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }

            let has_multiple_upstreams = route.upstream.len() > 1;
            let has_weights = route.upstream.iter().any(|u| u.weight > 1);
            let addrs: Vec<&str> = route.upstream.iter().map(|u| u.address.as_str()).collect();

            match &route.tls {
                TlsConfig::None => {
                    output.push_str(&format!("http://{} {{\n", route.domain));
                }
                TlsConfig::Auto { email } => {
                    output.push_str(&format!("{} {{\n", route.domain));
                    output.push_str(&format!("    tls {email}\n"));
                }
                TlsConfig::Manual { cert, key } => {
                    output.push_str(&format!("{} {{\n", route.domain));
                    output.push_str(&format!("    tls {} {}\n", cert.display(), key.display()));
                }
            }

            if has_multiple_upstreams {
                output.push_str(&format!("    reverse_proxy {} {{\n", addrs.join(" ")));
                if has_weights {
                    output.push_str("        lb_policy weighted_round_robin\n");
                }
                output.push_str("        health_uri /health\n");
                output.push_str("        health_interval 10s\n");
                output.push_str("    }\n");
            } else {
                output.push_str(&format!("    reverse_proxy {}\n", addrs[0]));
            }

            output.push_str("}\n");
        }

        output
    }
}

#[async_trait]
impl ProxyBackend for CaddyBackend {
    async fn apply_routes(&self, routes: &[RouteConfig]) -> Result<()> {
        let content = Self::render_caddyfile(routes);
        tokio::fs::write(&self.caddyfile_path, &content)
            .await
            .map_err(|e| {
                NexaError::Proxy(format!(
                    "failed to write Caddyfile {}: {e}",
                    self.caddyfile_path.display()
                ))
            })?;
        info!(path = %self.caddyfile_path.display(), "wrote Caddyfile");
        Ok(())
    }

    async fn remove_route(&self, domain: &str) -> Result<()> {
        let content = match tokio::fs::read_to_string(&self.caddyfile_path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                warn!(domain, "Caddyfile not found, nothing to remove");
                return Ok(());
            }
            Err(e) => {
                return Err(NexaError::Proxy(format!("failed to read Caddyfile: {e}")));
            }
        };

        // We need to find and remove the block that starts with this domain.
        // The domain may appear as "http://domain {" or "domain {".
        let mut result = String::new();
        let mut skip = false;
        let mut brace_depth: i32 = 0;

        for line in content.lines() {
            if !skip {
                let trimmed = line.trim();
                // Match the domain block header: "domain {", "http://domain {", etc.
                let is_domain_line = trimmed.starts_with(&format!("{domain} "))
                    || trimmed.starts_with(&format!("http://{domain} "))
                    || trimmed.starts_with(&format!("https://{domain} "))
                    || trimmed == domain
                    || trimmed == format!("http://{domain}")
                    || trimmed == format!("https://{domain}");

                if is_domain_line && trimmed.ends_with('{') {
                    skip = true;
                    brace_depth = 1;
                    continue;
                }
            }

            if skip {
                for ch in line.chars() {
                    match ch {
                        '{' => brace_depth += 1,
                        '}' => brace_depth -= 1,
                        _ => {}
                    }
                }
                if brace_depth <= 0 {
                    skip = false;
                }
                continue;
            }

            result.push_str(line);
            result.push('\n');
        }

        // Trim trailing whitespace but keep a final newline if content is non-empty
        let trimmed = result.trim_end().to_string();
        let final_content = if trimmed.is_empty() {
            String::new()
        } else {
            format!("{trimmed}\n")
        };

        tokio::fs::write(&self.caddyfile_path, &final_content)
            .await
            .map_err(|e| NexaError::Proxy(format!("failed to rewrite Caddyfile: {e}")))?;
        info!(domain, "removed route from Caddyfile");
        Ok(())
    }

    async fn reload(&self) -> Result<()> {
        let content = tokio::fs::read_to_string(&self.caddyfile_path)
            .await
            .map_err(|e| NexaError::Proxy(format!("failed to read Caddyfile for reload: {e}")))?;

        let url = format!("{}/load", self.admin_api);
        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .header("Content-Type", "text/caddyfile")
            .body(content)
            .send()
            .await
            .map_err(|e| NexaError::Proxy(format!("caddy reload request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(NexaError::Proxy(format!(
                "caddy reload returned {status}: {body}"
            )));
        }
        info!("caddy configuration reloaded via admin API");
        Ok(())
    }

    async fn health(&self) -> Result<bool> {
        let url = format!("{}/config/", self.admin_api);
        let client = reqwest::Client::new();
        match client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexa_core::ports::proxy::Upstream;

    fn auto_tls_route() -> RouteConfig {
        RouteConfig {
            domain: "app.example.com".into(),
            upstream: vec![Upstream {
                address: "10.0.0.1:8080".into(),
                weight: 1,
            }],
            tls: TlsConfig::Auto {
                email: "admin@example.com".into(),
            },
        }
    }

    fn no_tls_route() -> RouteConfig {
        RouteConfig {
            domain: "plain.example.com".into(),
            upstream: vec![Upstream {
                address: "10.0.0.2:3000".into(),
                weight: 1,
            }],
            tls: TlsConfig::None,
        }
    }

    fn manual_tls_route() -> RouteConfig {
        RouteConfig {
            domain: "manual.example.com".into(),
            upstream: vec![Upstream {
                address: "10.0.0.3:443".into(),
                weight: 1,
            }],
            tls: TlsConfig::Manual {
                cert: PathBuf::from("/certs/cert.pem"),
                key: PathBuf::from("/certs/key.pem"),
            },
        }
    }

    fn multi_upstream_route() -> RouteConfig {
        RouteConfig {
            domain: "balanced.example.com".into(),
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
                email: "ops@example.com".into(),
            },
        }
    }

    #[test]
    fn render_caddyfile_auto_tls() {
        let out = CaddyBackend::render_caddyfile(&[auto_tls_route()]);
        assert!(out.contains("app.example.com {"));
        assert!(out.contains("tls admin@example.com"));
        assert!(out.contains("reverse_proxy 10.0.0.1:8080"));
        // Should NOT have http:// prefix for auto TLS
        assert!(!out.contains("http://app.example.com"));
    }

    #[test]
    fn render_caddyfile_no_tls() {
        let out = CaddyBackend::render_caddyfile(&[no_tls_route()]);
        assert!(out.contains("http://plain.example.com {"));
        assert!(out.contains("reverse_proxy 10.0.0.2:3000"));
        // No tls directive
        assert!(!out.contains("tls "));
    }

    #[test]
    fn render_caddyfile_manual_tls() {
        let out = CaddyBackend::render_caddyfile(&[manual_tls_route()]);
        assert!(out.contains("manual.example.com {"));
        assert!(out.contains("tls /certs/cert.pem /certs/key.pem"));
        assert!(out.contains("reverse_proxy 10.0.0.3:443"));
    }

    #[test]
    fn render_caddyfile_multiple_upstreams() {
        let out = CaddyBackend::render_caddyfile(&[multi_upstream_route()]);
        assert!(out.contains("reverse_proxy 10.0.0.1:8080 10.0.0.2:8080 {"));
        assert!(out.contains("lb_policy weighted_round_robin"));
        assert!(out.contains("health_uri /health"));
        assert!(out.contains("health_interval 10s"));
    }

    #[tokio::test]
    async fn apply_routes_writes_caddyfile() {
        let dir = tempfile::tempdir().unwrap();
        let caddyfile = dir.path().join("Caddyfile");
        let backend = CaddyBackend::new(caddyfile.clone(), "http://localhost:2019".into());

        backend
            .apply_routes(&[auto_tls_route(), no_tls_route()])
            .await
            .unwrap();

        assert!(caddyfile.exists());
        let content = tokio::fs::read_to_string(&caddyfile).await.unwrap();
        assert!(content.contains("app.example.com"));
        assert!(content.contains("http://plain.example.com"));
    }

    #[tokio::test]
    async fn remove_route_strips_block() {
        let dir = tempfile::tempdir().unwrap();
        let caddyfile = dir.path().join("Caddyfile");
        let backend = CaddyBackend::new(caddyfile.clone(), "http://localhost:2019".into());

        // Write two routes
        backend
            .apply_routes(&[auto_tls_route(), no_tls_route()])
            .await
            .unwrap();

        // Remove the first route
        backend.remove_route("app.example.com").await.unwrap();

        let content = tokio::fs::read_to_string(&caddyfile).await.unwrap();
        assert!(!content.contains("app.example.com"));
        assert!(content.contains("http://plain.example.com"));
    }
}
