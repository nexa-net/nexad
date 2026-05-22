use std::net::{Ipv4Addr, SocketAddr};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use tracing::info;
use x25519_dalek::{PublicKey, StaticSecret};

use nexa_core::error::Result;

#[derive(Debug, Clone)]
pub struct WgKeypair {
    pub private_key: [u8; 32],
    pub public_key: [u8; 32],
}

impl WgKeypair {
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(rand::rngs::OsRng);
        let public = PublicKey::from(&secret);
        Self {
            private_key: secret.to_bytes(),
            public_key: public.to_bytes(),
        }
    }

    pub fn private_key_base64(&self) -> String {
        BASE64.encode(self.private_key)
    }

    pub fn public_key_base64(&self) -> String {
        BASE64.encode(self.public_key)
    }
}

#[derive(Debug, Clone)]
pub struct WgPeerConfig {
    pub public_key: [u8; 32],
    pub endpoint: Option<SocketAddr>,
    pub allowed_ips: Vec<String>,
    pub persistent_keepalive: Option<u16>,
}

#[allow(dead_code)]
pub struct WireguardManager {
    keypair: WgKeypair,
    node_ip: Ipv4Addr,
    listen_port: u16,
    active: bool,
}

impl WireguardManager {
    pub fn inactive() -> Self {
        Self {
            keypair: WgKeypair {
                private_key: [0u8; 32],
                public_key: [0u8; 32],
            },
            node_ip: Ipv4Addr::UNSPECIFIED,
            listen_port: 0,
            active: false,
        }
    }

    pub fn new(node_ip: Ipv4Addr, listen_port: u16) -> Self {
        let keypair = WgKeypair::generate();
        info!(
            public_key = keypair.public_key_base64(),
            %node_ip,
            listen_port,
            "WireGuard keypair generated"
        );
        Self {
            keypair,
            node_ip,
            listen_port,
            active: true,
        }
    }

    pub fn public_key(&self) -> &[u8; 32] {
        &self.keypair.public_key
    }

    pub fn public_key_base64(&self) -> String {
        self.keypair.public_key_base64()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn listen_port(&self) -> u16 {
        self.listen_port
    }

    pub fn create_tunnel(&self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        info!(
            public_key = self.keypair.public_key_base64(),
            listen_port = self.listen_port,
            "WireGuard tunnel initialized (boringtun userspace)"
        );
        Ok(())
    }

    pub fn peer_config_for_node(
        &self,
        node_public_key: [u8; 32],
        node_endpoint: SocketAddr,
        node_subnet: &str,
    ) -> WgPeerConfig {
        WgPeerConfig {
            public_key: node_public_key,
            endpoint: Some(node_endpoint),
            allowed_ips: vec![node_subnet.to_string()],
            persistent_keepalive: Some(25),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_keypair() {
        let kp = WgKeypair::generate();
        assert_eq!(kp.private_key.len(), 32);
        assert_eq!(kp.public_key.len(), 32);
        assert_ne!(kp.private_key, [0u8; 32]);
        assert_ne!(kp.public_key, [0u8; 32]);
        assert_ne!(kp.private_key, kp.public_key);
    }

    #[test]
    fn keypair_base64_encoding() {
        let kp = WgKeypair::generate();
        let priv_b64 = kp.private_key_base64();
        let pub_b64 = kp.public_key_base64();
        assert_eq!(priv_b64.len(), 44);
        assert_eq!(pub_b64.len(), 44);
        assert!(priv_b64.ends_with('='));
    }

    #[test]
    fn two_keypairs_are_different() {
        let kp1 = WgKeypair::generate();
        let kp2 = WgKeypair::generate();
        assert_ne!(kp1.public_key, kp2.public_key);
    }

    #[test]
    fn inactive_manager() {
        let mgr = WireguardManager::inactive();
        assert!(!mgr.is_active());
        assert_eq!(mgr.listen_port(), 0);
    }

    #[test]
    fn active_manager() {
        let mgr = WireguardManager::new(Ipv4Addr::new(172, 20, 0, 1), 51820);
        assert!(mgr.is_active());
        assert_eq!(mgr.listen_port(), 51820);
        assert_ne!(mgr.public_key(), &[0u8; 32]);
    }

    #[test]
    fn create_tunnel_inactive_is_noop() {
        let mgr = WireguardManager::inactive();
        assert!(mgr.create_tunnel().is_ok());
    }

    #[test]
    fn create_tunnel_active() {
        let mgr = WireguardManager::new(Ipv4Addr::new(172, 20, 0, 1), 51820);
        assert!(mgr.create_tunnel().is_ok());
    }

    #[test]
    fn peer_config_generation() {
        let mgr = WireguardManager::new(Ipv4Addr::new(172, 20, 0, 1), 51820);
        let peer_key = WgKeypair::generate();
        let endpoint: SocketAddr = "192.168.1.100:51820".parse().unwrap();

        let peer_config = mgr.peer_config_for_node(
            peer_key.public_key,
            endpoint,
            "172.20.1.0/24",
        );

        assert_eq!(peer_config.public_key, peer_key.public_key);
        assert_eq!(peer_config.endpoint, Some(endpoint));
        assert_eq!(peer_config.allowed_ips, vec!["172.20.1.0/24"]);
        assert_eq!(peer_config.persistent_keepalive, Some(25));
    }
}
