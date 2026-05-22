use std::net::IpAddr;

use async_trait::async_trait;

use nexa_core::error::Result;
use nexa_core::ports::dns::DnsProvider;

pub struct NoopDnsProvider;

impl NoopDnsProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DnsProvider for NoopDnsProvider {
    async fn register(&self, _project: &str, _deployment: &str, _ip: IpAddr) -> Result<()> {
        Ok(())
    }

    async fn deregister(&self, _project: &str, _deployment: &str, _ip: IpAddr) -> Result<()> {
        Ok(())
    }

    async fn lookup(&self, _project: &str, _deployment: &str) -> Result<Vec<IpAddr>> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn register_returns_ok() {
        let dns = NoopDnsProvider::new();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        assert!(dns.register("myapp", "api", ip).await.is_ok());
    }

    #[tokio::test]
    async fn deregister_returns_ok() {
        let dns = NoopDnsProvider::new();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        assert!(dns.deregister("myapp", "api", ip).await.is_ok());
    }

    #[tokio::test]
    async fn lookup_returns_empty() {
        let dns = NoopDnsProvider::new();
        let result = dns.lookup("myapp", "api").await.unwrap();
        assert!(result.is_empty());
    }
}
