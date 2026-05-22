use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::net::{TcpListener, UdpSocket};
use tracing::{error, info};

use nexa_core::error::{NexaError, Result};
use nexa_core::ports::dns::DnsProvider;

use super::record_store::DnsRecordStore;

pub struct HickoryDnsProvider {
    store: Arc<DnsRecordStore>,
    listen_addr: SocketAddr,
    upstream_dns: SocketAddr,
}

impl HickoryDnsProvider {
    pub fn new(listen_addr: SocketAddr, upstream_dns: SocketAddr) -> Self {
        Self {
            store: Arc::new(DnsRecordStore::new()),
            listen_addr,
            upstream_dns,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let store = self.store.clone();
        let listen_addr = self.listen_addr;
        let upstream_dns = self.upstream_dns;

        let udp_socket = UdpSocket::bind(listen_addr)
            .await
            .map_err(|e| NexaError::Runtime(format!("failed to bind DNS UDP on {listen_addr}: {e}")))?;

        let tcp_listener = TcpListener::bind(listen_addr)
            .await
            .map_err(|e| NexaError::Runtime(format!("failed to bind DNS TCP on {listen_addr}: {e}")))?;

        info!(%listen_addr, %upstream_dns, "starting embedded DNS server");

        let udp_store = store.clone();
        let udp_upstream = upstream_dns;
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            loop {
                match udp_socket.recv_from(&mut buf).await {
                    Ok((len, src)) => {
                        let data = buf[..len].to_vec();
                        let response = handle_dns_query(&data, &udp_store, udp_upstream).await;
                        if let Some(response_bytes) = response {
                            if let Err(e) = udp_socket.send_to(&response_bytes, src).await {
                                error!(%e, "failed to send DNS UDP response");
                            }
                        }
                    }
                    Err(e) => {
                        error!(%e, "DNS UDP recv error");
                    }
                }
            }
        });

        let tcp_store = store.clone();
        let tcp_upstream = upstream_dns;
        tokio::spawn(async move {
            loop {
                match tcp_listener.accept().await {
                    Ok((stream, _addr)) => {
                        let store = tcp_store.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_tcp_dns_client(stream, &store, tcp_upstream).await {
                                error!(%e, "DNS TCP handler error");
                            }
                        });
                    }
                    Err(e) => {
                        error!(%e, "DNS TCP accept error");
                    }
                }
            }
        });

        Ok(())
    }

    pub fn store(&self) -> &Arc<DnsRecordStore> {
        &self.store
    }
}

#[async_trait]
impl DnsProvider for HickoryDnsProvider {
    async fn register(&self, project: &str, deployment: &str, ip: IpAddr) -> Result<()> {
        self.store.register(project, deployment, ip);
        info!(project, deployment, %ip, "DNS record registered");
        Ok(())
    }

    async fn deregister(&self, project: &str, deployment: &str, ip: IpAddr) -> Result<()> {
        self.store.deregister(project, deployment, ip);
        info!(project, deployment, %ip, "DNS record deregistered");
        Ok(())
    }

    async fn lookup(&self, project: &str, deployment: &str) -> Result<Vec<IpAddr>> {
        Ok(self.store.lookup(project, deployment))
    }
}

async fn handle_tcp_dns_client(
    mut stream: tokio::net::TcpStream,
    store: &DnsRecordStore,
    upstream: SocketAddr,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await?;
    let msg_len = u16::from_be_bytes(len_buf) as usize;

    let mut msg_buf = vec![0u8; msg_len];
    stream.read_exact(&mut msg_buf).await?;

    if let Some(response) = handle_dns_query(&msg_buf, store, upstream).await {
        let resp_len = (response.len() as u16).to_be_bytes();
        stream.write_all(&resp_len).await?;
        stream.write_all(&response).await?;
    }

    Ok(())
}

async fn handle_dns_query(
    data: &[u8],
    store: &DnsRecordStore,
    upstream: SocketAddr,
) -> Option<Vec<u8>> {
    if data.len() < 12 {
        return None;
    }

    let id = u16::from_be_bytes([data[0], data[1]]);
    let flags = u16::from_be_bytes([data[2], data[3]]);
    let qd_count = u16::from_be_bytes([data[4], data[5]]);

    let opcode = (flags >> 11) & 0xF;
    if opcode != 0 || qd_count == 0 {
        return None;
    }

    let (query_name, qtype, question_end) = parse_question(data, 12)?;

    let name_lower = query_name.to_lowercase();
    if name_lower.ends_with(".internal") || name_lower.ends_with(".internal.") {
        let ips = store.resolve(&name_lower);
        return Some(build_dns_response(id, data, &query_name, question_end, qtype, ips));
    }

    forward_to_upstream(data, upstream).await
}

fn parse_question(data: &[u8], mut offset: usize) -> Option<(String, u16, usize)> {
    let mut labels = Vec::new();

    loop {
        if offset >= data.len() {
            return None;
        }
        let label_len = data[offset] as usize;
        offset += 1;

        if label_len == 0 {
            break;
        }

        if offset + label_len > data.len() {
            return None;
        }

        let label = std::str::from_utf8(&data[offset..offset + label_len]).ok()?;
        labels.push(label.to_string());
        offset += label_len;
    }

    if offset + 4 > data.len() {
        return None;
    }

    let qtype = u16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 4;

    let name = labels.join(".");
    Some((name, qtype, offset))
}

fn build_dns_response(
    id: u16,
    original: &[u8],
    _query_name: &str,
    question_end: usize,
    qtype: u16,
    ips: Option<Vec<IpAddr>>,
) -> Vec<u8> {
    let ips = ips.unwrap_or_default();

    let matching_ips: Vec<&IpAddr> = ips
        .iter()
        .filter(|ip| match (qtype, ip) {
            (1, IpAddr::V4(_)) => true,
            (28, IpAddr::V6(_)) => true,
            (255, _) => true,
            _ => false,
        })
        .collect();

    let an_count = matching_ips.len() as u16;
    let rcode = if matching_ips.is_empty() && ips.is_empty() { 3u16 } else { 0u16 };

    let flags: u16 = 0x8000 | 0x0400 | 0x0080 | rcode;

    let mut response = Vec::with_capacity(512);

    response.extend_from_slice(&id.to_be_bytes());
    response.extend_from_slice(&flags.to_be_bytes());
    response.extend_from_slice(&1u16.to_be_bytes());
    response.extend_from_slice(&an_count.to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes());

    response.extend_from_slice(&original[12..question_end]);

    for ip in &matching_ips {
        response.extend_from_slice(&[0xC0, 0x0C]);

        match ip {
            IpAddr::V4(v4) => {
                response.extend_from_slice(&1u16.to_be_bytes());
                response.extend_from_slice(&1u16.to_be_bytes());
                response.extend_from_slice(&60u32.to_be_bytes());
                response.extend_from_slice(&4u16.to_be_bytes());
                response.extend_from_slice(&v4.octets());
            }
            IpAddr::V6(v6) => {
                response.extend_from_slice(&28u16.to_be_bytes());
                response.extend_from_slice(&1u16.to_be_bytes());
                response.extend_from_slice(&60u32.to_be_bytes());
                response.extend_from_slice(&16u16.to_be_bytes());
                response.extend_from_slice(&v6.octets());
            }
        }
    }

    response
}

async fn forward_to_upstream(data: &[u8], upstream: SocketAddr) -> Option<Vec<u8>> {
    let socket = UdpSocket::bind("0.0.0.0:0").await.ok()?;
    socket.send_to(data, upstream).await.ok()?;

    let mut buf = vec![0u8; 4096];
    match tokio::time::timeout(Duration::from_secs(5), socket.recv_from(&mut buf)).await {
        Ok(Ok((len, _))) => Some(buf[..len].to_vec()),
        _ => {
            error!("upstream DNS timeout");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn ip4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    #[tokio::test]
    async fn register_and_lookup_via_provider() {
        let provider = HickoryDnsProvider::new(
            "127.0.0.1:15353".parse().unwrap(),
            "8.8.8.8:53".parse().unwrap(),
        );
        provider.register("ecommerce", "api", ip4(10, 0, 0, 1)).await.unwrap();
        provider.register("ecommerce", "api", ip4(10, 0, 0, 2)).await.unwrap();
        let ips = provider.lookup("ecommerce", "api").await.unwrap();
        assert_eq!(ips, vec![ip4(10, 0, 0, 1), ip4(10, 0, 0, 2)]);
    }

    #[tokio::test]
    async fn deregister_removes_via_provider() {
        let provider = HickoryDnsProvider::new(
            "127.0.0.1:15354".parse().unwrap(),
            "8.8.8.8:53".parse().unwrap(),
        );
        provider.register("app", "web", ip4(10, 0, 0, 1)).await.unwrap();
        provider.deregister("app", "web", ip4(10, 0, 0, 1)).await.unwrap();
        let ips = provider.lookup("app", "web").await.unwrap();
        assert!(ips.is_empty());
    }

    #[test]
    fn parse_question_extracts_name_and_type() {
        let mut data = vec![
            0x00, 0x01, 0x01, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.push(3); data.extend_from_slice(b"api");
        data.push(9); data.extend_from_slice(b"ecommerce");
        data.push(8); data.extend_from_slice(b"internal");
        data.push(0);
        data.extend_from_slice(&[0x00, 0x01]);
        data.extend_from_slice(&[0x00, 0x01]);

        let (name, qtype, _end) = parse_question(&data, 12).unwrap();
        assert_eq!(name, "api.ecommerce.internal");
        assert_eq!(qtype, 1);
    }

    #[test]
    fn build_response_with_ips() {
        let mut query = vec![
            0x00, 0x42, 0x01, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        query.push(3); query.extend_from_slice(b"api");
        query.push(9); query.extend_from_slice(b"ecommerce");
        query.push(8); query.extend_from_slice(b"internal");
        query.push(0);
        query.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);

        let question_end = query.len();
        let ips = Some(vec![ip4(10, 0, 0, 1), ip4(10, 0, 0, 2)]);

        let response = build_dns_response(0x0042, &query, "api.ecommerce.internal", question_end, 1, ips);

        assert_eq!(response[0..2], [0x00, 0x42]);
        let flags = u16::from_be_bytes([response[2], response[3]]);
        assert!(flags & 0x8000 != 0);
        assert!(flags & 0x0400 != 0);
        let an_count = u16::from_be_bytes([response[6], response[7]]);
        assert_eq!(an_count, 2);
    }

    #[test]
    fn build_response_nxdomain_when_no_ips() {
        let mut query = vec![
            0x00, 0x01, 0x01, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        query.push(3); query.extend_from_slice(b"xxx");
        query.push(3); query.extend_from_slice(b"yyy");
        query.push(8); query.extend_from_slice(b"internal");
        query.push(0);
        query.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);

        let question_end = query.len();

        let response = build_dns_response(0x0001, &query, "xxx.yyy.internal", question_end, 1, None);
        let flags = u16::from_be_bytes([response[2], response[3]]);
        let rcode = flags & 0x000F;
        assert_eq!(rcode, 3);
    }
}
