use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

/// Manages CNI network configurations for containerd.
pub struct CniManager {
    cni_bin_dir: PathBuf,
    cni_conf_dir: PathBuf,
    subnet_allocator: SubnetAllocator,
}

/// A CNI conflist configuration.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CniConfig {
    pub cni_version: String,
    pub name: String,
    pub plugins: Vec<CniPlugin>,
}

/// CNI plugin types supported by NexaNet.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CniPlugin {
    Bridge {
        bridge: String,
        #[serde(rename = "isGateway")]
        is_gateway: bool,
        ipam: CniIpam,
    },
    Loopback {},
}

/// IPAM configuration for CNI plugins.
#[derive(Debug, Clone, Serialize)]
pub struct CniIpam {
    #[serde(rename = "type")]
    pub ipam_type: String,
    pub subnet: String,
}

/// Allocates /24 subnets from the 172.20.0.0/16 range sequentially.
pub struct SubnetAllocator {
    allocations: Mutex<HashMap<String, u8>>,
    next_octet: Mutex<u8>,
}

impl SubnetAllocator {
    pub fn new() -> Self {
        Self {
            allocations: Mutex::new(HashMap::new()),
            next_octet: Mutex::new(1),
        }
    }

    /// Allocate a /24 subnet for the given network name.
    /// Returns the same subnet if the name was already allocated.
    pub fn allocate(&self, name: &str) -> nexa_core::error::Result<String> {
        let mut allocs = self.allocations.lock().unwrap();
        if let Some(&octet) = allocs.get(name) {
            return Ok(format!("172.20.{octet}.0/24"));
        }
        let mut next = self.next_octet.lock().unwrap();
        if *next == 0 {
            return Err(nexa_core::error::NexaError::Runtime(
                "subnet pool exhausted (255 /24 networks allocated)".into(),
            ));
        }
        let octet = *next;
        *next = next.wrapping_add(1);
        allocs.insert(name.to_string(), octet);
        Ok(format!("172.20.{octet}.0/24"))
    }

    /// Release a subnet allocation for the given network name.
    pub fn release(&self, name: &str) {
        let mut allocs = self.allocations.lock().unwrap();
        allocs.remove(name);
    }
}

impl CniManager {
    /// Create a new CniManager rooted under the given data directory.
    pub fn new(data_dir: &str) -> Self {
        let base = PathBuf::from(data_dir);
        Self {
            cni_bin_dir: base.join("cni").join("bin"),
            cni_conf_dir: base.join("cni").join("conf"),
            subnet_allocator: SubnetAllocator::new(),
        }
    }

    /// Check that required CNI plugins (bridge, loopback) exist in the bin directory.
    pub fn check_plugins(&self) -> anyhow::Result<()> {
        for plugin in &["bridge", "loopback"] {
            let path = self.cni_bin_dir.join(plugin);
            if !path.exists() {
                anyhow::bail!("CNI plugin not found: {}", path.display());
            }
        }
        Ok(())
    }

    /// Ensure a CNI network configuration exists for the given name.
    /// Creates the conflist file if it doesn't exist; idempotent.
    pub fn ensure_network(&self, name: &str) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.cni_conf_dir)?;

        let conf_path = self.cni_conf_dir.join(format!("{name}.conflist"));
        if conf_path.exists() {
            return Ok(());
        }

        let subnet = self.subnet_allocator.allocate(name)?;
        let config = CniConfig {
            cni_version: "1.0.0".to_string(),
            name: name.to_string(),
            plugins: vec![
                CniPlugin::Bridge {
                    bridge: format!("cni-{name}"),
                    is_gateway: true,
                    ipam: CniIpam {
                        ipam_type: "host-local".to_string(),
                        subnet,
                    },
                },
                CniPlugin::Loopback {},
            ],
        };

        let json = serde_json::to_string_pretty(&config)?;
        std::fs::write(&conf_path, json)?;
        Ok(())
    }

    /// Remove a CNI network configuration.
    pub fn remove_network(&self, name: &str) -> anyhow::Result<()> {
        let conf_path = self.cni_conf_dir.join(format!("{name}.conflist"));
        if conf_path.exists() {
            std::fs::remove_file(&conf_path)?;
        }
        self.subnet_allocator.release(name);
        Ok(())
    }

    /// Attach a container to a CNI network (placeholder — requires CNI plugin exec).
    pub fn attach(&self, _container_id: &str, _network: &str) -> anyhow::Result<String> {
        anyhow::bail!("CNI attach not yet implemented")
    }

    /// Detach a container from a CNI network (placeholder — requires CNI plugin exec).
    pub fn detach(&self, _container_id: &str, _network: &str) -> anyhow::Result<()> {
        anyhow::bail!("CNI detach not yet implemented")
    }

    /// Return the CNI binary directory path.
    pub fn bin_dir(&self) -> &Path {
        &self.cni_bin_dir
    }

    /// Return the CNI configuration directory path.
    pub fn conf_dir(&self) -> &Path {
        &self.cni_conf_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subnet_allocator_assigns_sequential_subnets() {
        let alloc = SubnetAllocator::new();
        let s1 = alloc.allocate("net-a").unwrap();
        let s2 = alloc.allocate("net-b").unwrap();
        assert_eq!(s1, "172.20.1.0/24");
        assert_eq!(s2, "172.20.2.0/24");
    }

    #[test]
    fn subnet_allocator_returns_same_for_existing() {
        let alloc = SubnetAllocator::new();
        let s1 = alloc.allocate("net-a").unwrap();
        let s2 = alloc.allocate("net-a").unwrap();
        assert_eq!(s1, s2);
    }

    #[test]
    fn subnet_allocator_release_frees_name() {
        let alloc = SubnetAllocator::new();
        let s1 = alloc.allocate("net-a").unwrap();
        assert_eq!(s1, "172.20.1.0/24");
        alloc.release("net-a");
        // After release, a new allocation for the same name gets the next octet,
        // not the freed one (sequential allocator).
        let s2 = alloc.allocate("net-a").unwrap();
        assert_eq!(s2, "172.20.2.0/24");
    }

    #[test]
    fn ensure_network_creates_conflist() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = CniManager::new(dir.path().to_str().unwrap());

        mgr.ensure_network("test-net").unwrap();

        let conf_path = mgr.conf_dir().join("test-net.conflist");
        assert!(conf_path.exists());

        let content = std::fs::read_to_string(&conf_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["name"], "test-net");
        assert_eq!(parsed["cniVersion"], "1.0.0");
        assert_eq!(parsed["plugins"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn ensure_network_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = CniManager::new(dir.path().to_str().unwrap());

        mgr.ensure_network("test-net").unwrap();
        let content1 = std::fs::read_to_string(mgr.conf_dir().join("test-net.conflist")).unwrap();

        mgr.ensure_network("test-net").unwrap();
        let content2 = std::fs::read_to_string(mgr.conf_dir().join("test-net.conflist")).unwrap();

        assert_eq!(content1, content2);
    }

    #[test]
    fn remove_network_deletes_conflist() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = CniManager::new(dir.path().to_str().unwrap());

        mgr.ensure_network("test-net").unwrap();
        let conf_path = mgr.conf_dir().join("test-net.conflist");
        assert!(conf_path.exists());

        mgr.remove_network("test-net").unwrap();
        assert!(!conf_path.exists());
    }

    #[test]
    fn remove_network_noop_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = CniManager::new(dir.path().to_str().unwrap());

        // Should not error even if the conflist doesn't exist.
        mgr.remove_network("nonexistent").unwrap();
    }

    #[test]
    fn check_plugins_fails_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = CniManager::new(dir.path().to_str().unwrap());

        let result = mgr.check_plugins();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CNI plugin not found"));
    }

    #[test]
    fn check_plugins_passes_when_present() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = CniManager::new(dir.path().to_str().unwrap());

        // Create fake plugin binaries.
        std::fs::create_dir_all(mgr.bin_dir()).unwrap();
        std::fs::write(mgr.bin_dir().join("bridge"), b"fake").unwrap();
        std::fs::write(mgr.bin_dir().join("loopback"), b"fake").unwrap();

        mgr.check_plugins().unwrap();
    }

    #[test]
    fn multiple_networks_get_different_subnets() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = CniManager::new(dir.path().to_str().unwrap());

        mgr.ensure_network("alpha").unwrap();
        mgr.ensure_network("beta").unwrap();

        let alpha_content = std::fs::read_to_string(mgr.conf_dir().join("alpha.conflist")).unwrap();
        let beta_content = std::fs::read_to_string(mgr.conf_dir().join("beta.conflist")).unwrap();

        let alpha: serde_json::Value = serde_json::from_str(&alpha_content).unwrap();
        let beta: serde_json::Value = serde_json::from_str(&beta_content).unwrap();

        let alpha_subnet = alpha["plugins"][0]["ipam"]["subnet"].as_str().unwrap();
        let beta_subnet = beta["plugins"][0]["ipam"]["subnet"].as_str().unwrap();

        assert_ne!(alpha_subnet, beta_subnet);
        assert_eq!(alpha_subnet, "172.20.1.0/24");
        assert_eq!(beta_subnet, "172.20.2.0/24");
    }
}
