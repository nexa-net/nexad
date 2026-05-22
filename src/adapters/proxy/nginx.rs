use std::path::PathBuf;

use async_trait::async_trait;
use tracing::{info, warn};

use nexa_core::error::{NexaError, Result};
use nexa_core::ports::proxy::{ProxyBackend, RouteConfig, TlsConfig};

pub struct NginxBackend {
    conf_dir: PathBuf,
    nginx_bin: String,
}

impl NginxBackend {
    pub fn new(conf_dir: PathBuf, nginx_bin: String) -> Self {
        Self {
            conf_dir,
            nginx_bin,
        }
    }

    fn conf_path(&self, domain: &str) -> PathBuf {
        self.conf_dir.join(format!("nexa-{domain}.conf"))
    }

    fn render_config(route: &RouteConfig) -> String {
        let domain_underscored = route.domain.replace('.', "_").replace('-', "_");
        let mut cfg = String::new();

        // Upstream block
        cfg.push_str(&format!("upstream {domain_underscored} {{\n"));
        for up in &route.upstream {
            if up.weight > 1 {
                cfg.push_str(&format!(
                    "    server {} weight={};\n",
                    up.address, up.weight
                ));
            } else {
                cfg.push_str(&format!("    server {};\n", up.address));
            }
        }
        cfg.push_str("}\n\n");

        let proxy_headers = "\
    proxy_set_header Host $host;\n\
    proxy_set_header X-Real-IP $remote_addr;\n\
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n\
    proxy_set_header X-Forwarded-Proto $scheme;\n";

        match &route.tls {
            TlsConfig::None => {
                cfg.push_str(&format!("server {{\n"));
                cfg.push_str("    listen 80;\n");
                cfg.push_str(&format!("    server_name {};\n\n", route.domain));
                cfg.push_str("    location / {\n");
                cfg.push_str(&format!(
                    "        proxy_pass http://{domain_underscored};\n"
                ));
                cfg.push_str(
                    &proxy_headers
                        .lines()
                        .map(|l| format!("    {l}\n"))
                        .collect::<String>(),
                );
                cfg.push_str("    }\n");
                cfg.push_str("}\n");
            }
            TlsConfig::Auto { email } => {
                // HTTP -> HTTPS redirect + ACME challenge
                cfg.push_str("server {\n");
                cfg.push_str("    listen 80;\n");
                cfg.push_str(&format!("    server_name {};\n\n", route.domain));
                cfg.push_str("    location /.well-known/acme-challenge/ {\n");
                cfg.push_str("        root /var/www/certbot;\n");
                cfg.push_str("    }\n\n");
                cfg.push_str("    location / {\n");
                cfg.push_str("        return 301 https://$host$request_uri;\n");
                cfg.push_str("    }\n");
                cfg.push_str("}\n\n");

                // HTTPS server
                cfg.push_str("server {\n");
                cfg.push_str("    listen 443 ssl;\n");
                cfg.push_str(&format!("    server_name {};\n\n", route.domain));
                cfg.push_str(&format!(
                    "    ssl_certificate /etc/letsencrypt/live/{}/fullchain.pem;\n",
                    route.domain
                ));
                cfg.push_str(&format!(
                    "    ssl_certificate_key /etc/letsencrypt/live/{}/privkey.pem;\n",
                    route.domain
                ));
                cfg.push_str(&format!("    # Managed by Certbot for {email}\n\n"));
                cfg.push_str("    location / {\n");
                cfg.push_str(&format!(
                    "        proxy_pass http://{domain_underscored};\n"
                ));
                cfg.push_str(
                    &proxy_headers
                        .lines()
                        .map(|l| format!("    {l}\n"))
                        .collect::<String>(),
                );
                cfg.push_str("    }\n");
                cfg.push_str("}\n");
            }
            TlsConfig::Manual { cert, key } => {
                // HTTP -> HTTPS redirect
                cfg.push_str("server {\n");
                cfg.push_str("    listen 80;\n");
                cfg.push_str(&format!("    server_name {};\n\n", route.domain));
                cfg.push_str("    location / {\n");
                cfg.push_str("        return 301 https://$host$request_uri;\n");
                cfg.push_str("    }\n");
                cfg.push_str("}\n\n");

                // HTTPS server
                cfg.push_str("server {\n");
                cfg.push_str("    listen 443 ssl;\n");
                cfg.push_str(&format!("    server_name {};\n\n", route.domain));
                cfg.push_str(&format!("    ssl_certificate {};\n", cert.display()));
                cfg.push_str(&format!("    ssl_certificate_key {};\n\n", key.display()));
                cfg.push_str("    location / {\n");
                cfg.push_str(&format!(
                    "        proxy_pass http://{domain_underscored};\n"
                ));
                cfg.push_str(
                    &proxy_headers
                        .lines()
                        .map(|l| format!("    {l}\n"))
                        .collect::<String>(),
                );
                cfg.push_str("    }\n");
                cfg.push_str("}\n");
            }
        }

        cfg
    }
}

#[async_trait]
impl ProxyBackend for NginxBackend {
    async fn apply_routes(&self, routes: &[RouteConfig]) -> Result<()> {
        for route in routes {
            let path = self.conf_path(&route.domain);
            let content = Self::render_config(route);
            tokio::fs::write(&path, &content).await.map_err(|e| {
                NexaError::Proxy(format!(
                    "failed to write nginx config {}: {e}",
                    path.display()
                ))
            })?;
            info!(domain = %route.domain, path = %path.display(), "wrote nginx config");
        }
        Ok(())
    }

    async fn remove_route(&self, domain: &str) -> Result<()> {
        let path = self.conf_path(domain);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {
                info!(domain, path = %path.display(), "removed nginx config");
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                warn!(domain, path = %path.display(), "nginx config not found, nothing to remove");
                Ok(())
            }
            Err(e) => Err(NexaError::Proxy(format!(
                "failed to remove nginx config {}: {e}",
                path.display()
            ))),
        }
    }

    async fn reload(&self) -> Result<()> {
        let output = std::process::Command::new(&self.nginx_bin)
            .args(["-s", "reload"])
            .output()
            .map_err(|e| NexaError::Proxy(format!("failed to run nginx reload: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(NexaError::Proxy(format!("nginx reload failed: {stderr}")));
        }
        info!("nginx reloaded");
        Ok(())
    }

    async fn health(&self) -> Result<bool> {
        let output = std::process::Command::new(&self.nginx_bin)
            .args(["-t"])
            .output()
            .map_err(|e| NexaError::Proxy(format!("failed to run nginx -t: {e}")))?;

        Ok(output.status.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexa_core::ports::proxy::Upstream;

    fn http_route() -> RouteConfig {
        RouteConfig {
            domain: "app.example.com".into(),
            upstream: vec![Upstream {
                address: "10.0.0.1:8080".into(),
                weight: 1,
            }],
            tls: TlsConfig::None,
        }
    }

    fn auto_tls_route() -> RouteConfig {
        RouteConfig {
            domain: "secure.example.com".into(),
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
        }
    }

    fn manual_tls_route() -> RouteConfig {
        RouteConfig {
            domain: "custom.example.com".into(),
            upstream: vec![Upstream {
                address: "10.0.0.5:443".into(),
                weight: 1,
            }],
            tls: TlsConfig::Manual {
                cert: PathBuf::from("/etc/certs/cert.pem"),
                key: PathBuf::from("/etc/certs/key.pem"),
            },
        }
    }

    #[test]
    fn render_http_config() {
        let cfg = NginxBackend::render_config(&http_route());
        assert!(cfg.contains("upstream app_example_com"));
        assert!(cfg.contains("listen 80;"));
        assert!(cfg.contains("server_name app.example.com;"));
        assert!(cfg.contains("proxy_pass http://app_example_com;"));
        assert!(cfg.contains("proxy_set_header Host"));
        assert!(cfg.contains("proxy_set_header X-Real-IP"));
        assert!(cfg.contains("proxy_set_header X-Forwarded-For"));
        assert!(cfg.contains("proxy_set_header X-Forwarded-Proto"));
        // No SSL directives for plain HTTP
        assert!(!cfg.contains("ssl"));
        assert!(!cfg.contains("443"));
    }

    #[test]
    fn render_auto_tls_config() {
        let cfg = NginxBackend::render_config(&auto_tls_route());
        // HTTP redirect
        assert!(cfg.contains("return 301 https://"));
        // ACME challenge
        assert!(cfg.contains(".well-known/acme-challenge"));
        // HTTPS listener
        assert!(cfg.contains("listen 443 ssl;"));
        // Let's Encrypt cert paths
        assert!(cfg.contains("/etc/letsencrypt/live/secure.example.com/fullchain.pem"));
        assert!(cfg.contains("/etc/letsencrypt/live/secure.example.com/privkey.pem"));
        // Email comment
        assert!(cfg.contains("admin@example.com"));
        // Weighted upstream
        assert!(cfg.contains("weight=3"));
    }

    #[test]
    fn render_manual_tls_config() {
        let cfg = NginxBackend::render_config(&manual_tls_route());
        assert!(cfg.contains("return 301 https://"));
        assert!(cfg.contains("listen 443 ssl;"));
        assert!(cfg.contains("ssl_certificate /etc/certs/cert.pem;"));
        assert!(cfg.contains("ssl_certificate_key /etc/certs/key.pem;"));
        assert!(cfg.contains("proxy_set_header Host"));
    }

    #[test]
    fn conf_path_uses_domain() {
        let backend = NginxBackend::new(PathBuf::from("/etc/nginx/conf.d"), "nginx".into());
        let path = backend.conf_path("api.example.com");
        assert_eq!(
            path,
            PathBuf::from("/etc/nginx/conf.d/nexa-api.example.com.conf")
        );
    }

    #[tokio::test]
    async fn apply_routes_writes_files() {
        let dir = tempfile::tempdir().unwrap();
        let backend = NginxBackend::new(dir.path().to_path_buf(), "nginx".into());
        let routes = vec![http_route()];

        backend.apply_routes(&routes).await.unwrap();

        let path = dir.path().join("nexa-app.example.com.conf");
        assert!(path.exists());
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("upstream app_example_com"));
    }

    #[tokio::test]
    async fn remove_route_deletes_file() {
        let dir = tempfile::tempdir().unwrap();
        let backend = NginxBackend::new(dir.path().to_path_buf(), "nginx".into());

        // First create the file
        backend.apply_routes(&[http_route()]).await.unwrap();
        let path = dir.path().join("nexa-app.example.com.conf");
        assert!(path.exists());

        // Then remove it
        backend.remove_route("app.example.com").await.unwrap();
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn remove_nonexistent_route_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let backend = NginxBackend::new(dir.path().to_path_buf(), "nginx".into());

        // Should succeed even though no config file exists
        let result = backend.remove_route("nonexistent.example.com").await;
        assert!(result.is_ok());
    }
}
