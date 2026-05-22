use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::RwLock;

#[derive(Default)]
pub struct DnsRecordStore {
    entries: RwLock<HashMap<String, HashMap<String, Vec<IpAddr>>>>,
}

impl DnsRecordStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, project: &str, deployment: &str, ip: IpAddr) {
        let mut entries = self.entries.write().unwrap();
        let project_map = entries.entry(project.to_string()).or_default();
        let ips = project_map.entry(deployment.to_string()).or_default();
        if !ips.contains(&ip) {
            ips.push(ip);
        }
    }

    pub fn deregister(&self, project: &str, deployment: &str, ip: IpAddr) {
        let mut entries = self.entries.write().unwrap();
        if let Some(project_map) = entries.get_mut(project) {
            if let Some(ips) = project_map.get_mut(deployment) {
                ips.retain(|existing| existing != &ip);
                if ips.is_empty() {
                    project_map.remove(deployment);
                }
            }
            if project_map.is_empty() {
                entries.remove(project);
            }
        }
    }

    pub fn lookup(&self, project: &str, deployment: &str) -> Vec<IpAddr> {
        let entries = self.entries.read().unwrap();
        entries
            .get(project)
            .and_then(|m| m.get(deployment))
            .cloned()
            .unwrap_or_default()
    }

    pub fn lookup_replica(&self, project: &str, deployment: &str, index: usize) -> Option<IpAddr> {
        let entries = self.entries.read().unwrap();
        entries
            .get(project)
            .and_then(|m| m.get(deployment))
            .and_then(|ips| ips.get(index).copied())
    }

    pub fn resolve(&self, query_name: &str) -> Option<Vec<IpAddr>> {
        let name = query_name.trim_end_matches('.');
        let parts: Vec<&str> = name.splitn(3, '.').collect();
        if parts.len() != 3 || parts[2] != "internal" {
            return None;
        }

        let project = parts[1];
        let host = parts[0];

        if let Some(dash_pos) = host.rfind('-') {
            let maybe_index = &host[(dash_pos + 1)..];
            if let Ok(index) = maybe_index.parse::<usize>() {
                let deployment = &host[..dash_pos];
                if let Some(ip) = self.lookup_replica(project, deployment, index) {
                    return Some(vec![ip]);
                }
            }
        }

        let ips = self.lookup(project, host);
        if ips.is_empty() { None } else { Some(ips) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn ip(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    #[test]
    fn register_and_lookup() {
        let store = DnsRecordStore::new();
        store.register("ecommerce", "api", ip(10, 0, 0, 1));
        store.register("ecommerce", "api", ip(10, 0, 0, 2));
        let result = store.lookup("ecommerce", "api");
        assert_eq!(result, vec![ip(10, 0, 0, 1), ip(10, 0, 0, 2)]);
    }

    #[test]
    fn register_ignores_duplicates() {
        let store = DnsRecordStore::new();
        store.register("app", "web", ip(10, 0, 0, 1));
        store.register("app", "web", ip(10, 0, 0, 1));
        assert_eq!(store.lookup("app", "web").len(), 1);
    }

    #[test]
    fn deregister_removes_ip() {
        let store = DnsRecordStore::new();
        store.register("app", "web", ip(10, 0, 0, 1));
        store.register("app", "web", ip(10, 0, 0, 2));
        store.deregister("app", "web", ip(10, 0, 0, 1));
        assert_eq!(store.lookup("app", "web"), vec![ip(10, 0, 0, 2)]);
    }

    #[test]
    fn deregister_cleans_up_empty_maps() {
        let store = DnsRecordStore::new();
        store.register("app", "web", ip(10, 0, 0, 1));
        store.deregister("app", "web", ip(10, 0, 0, 1));
        assert!(store.lookup("app", "web").is_empty());
        let entries = store.entries.read().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn deregister_nonexistent_is_noop() {
        let store = DnsRecordStore::new();
        store.deregister("app", "web", ip(10, 0, 0, 1));
    }

    #[test]
    fn lookup_missing_returns_empty() {
        let store = DnsRecordStore::new();
        assert!(store.lookup("nonexistent", "api").is_empty());
    }

    #[test]
    fn lookup_replica_by_index() {
        let store = DnsRecordStore::new();
        store.register("ecommerce", "api", ip(10, 0, 0, 1));
        store.register("ecommerce", "api", ip(10, 0, 0, 2));
        store.register("ecommerce", "api", ip(10, 0, 0, 3));
        assert_eq!(
            store.lookup_replica("ecommerce", "api", 0),
            Some(ip(10, 0, 0, 1))
        );
        assert_eq!(
            store.lookup_replica("ecommerce", "api", 2),
            Some(ip(10, 0, 0, 3))
        );
        assert_eq!(store.lookup_replica("ecommerce", "api", 5), None);
    }

    #[test]
    fn resolve_deployment_name() {
        let store = DnsRecordStore::new();
        store.register("ecommerce", "api", ip(10, 0, 0, 1));
        store.register("ecommerce", "api", ip(10, 0, 0, 2));
        let result = store.resolve("api.ecommerce.internal").unwrap();
        assert_eq!(result, vec![ip(10, 0, 0, 1), ip(10, 0, 0, 2)]);
    }

    #[test]
    fn resolve_deployment_name_with_trailing_dot() {
        let store = DnsRecordStore::new();
        store.register("ecommerce", "api", ip(10, 0, 0, 1));
        let result = store.resolve("api.ecommerce.internal.").unwrap();
        assert_eq!(result, vec![ip(10, 0, 0, 1)]);
    }

    #[test]
    fn resolve_specific_replica() {
        let store = DnsRecordStore::new();
        store.register("ecommerce", "api", ip(10, 0, 0, 1));
        store.register("ecommerce", "api", ip(10, 0, 0, 2));
        store.register("ecommerce", "api", ip(10, 0, 0, 3));
        let result = store.resolve("api-1.ecommerce.internal").unwrap();
        assert_eq!(result, vec![ip(10, 0, 0, 2)]);
    }

    #[test]
    fn resolve_non_internal_returns_none() {
        let store = DnsRecordStore::new();
        store.register("ecommerce", "api", ip(10, 0, 0, 1));
        assert!(store.resolve("api.ecommerce.com").is_none());
    }

    #[test]
    fn resolve_unknown_deployment_returns_none() {
        let store = DnsRecordStore::new();
        assert!(store.resolve("unknown.myapp.internal").is_none());
    }

    #[test]
    fn resolve_deployment_name_with_dashes() {
        let store = DnsRecordStore::new();
        store.register("ecommerce", "my-api", ip(10, 0, 0, 1));
        let result = store.resolve("my-api.ecommerce.internal").unwrap();
        assert_eq!(result, vec![ip(10, 0, 0, 1)]);
    }

    #[test]
    fn multiple_projects_isolated() {
        let store = DnsRecordStore::new();
        store.register("proj-a", "api", ip(10, 0, 0, 1));
        store.register("proj-b", "api", ip(10, 0, 0, 2));
        assert_eq!(
            store.resolve("api.proj-a.internal").unwrap(),
            vec![ip(10, 0, 0, 1)]
        );
        assert_eq!(
            store.resolve("api.proj-b.internal").unwrap(),
            vec![ip(10, 0, 0, 2)]
        );
    }
}
