use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU8, Ordering};

pub struct SubnetAllocator {
    base_first: u8,
    base_second: u8,
    next_third_octet: AtomicU8,
}

impl SubnetAllocator {
    pub fn new(cidr: &str) -> Option<Self> {
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() != 2 {
            return None;
        }
        let prefix_len: u8 = parts[1].parse().ok()?;
        if prefix_len != 16 {
            return None;
        }
        let ip: Ipv4Addr = parts[0].parse().ok()?;
        let octets = ip.octets();

        Some(Self {
            base_first: octets[0],
            base_second: octets[1],
            next_third_octet: AtomicU8::new(1),
        })
    }

    pub fn allocate(&self) -> Option<String> {
        let third = self.next_third_octet.fetch_add(1, Ordering::SeqCst);
        if third == 0 {
            return None;
        }
        Some(format!(
            "{}.{}.{}.0/24",
            self.base_first, self.base_second, third
        ))
    }

    pub fn master_gateway(&self) -> Ipv4Addr {
        Ipv4Addr::new(self.base_first, self.base_second, 0, 1)
    }

    pub fn master_subnet(&self) -> String {
        format!("{}.{}.0.0/24", self.base_first, self.base_second)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_valid_cidr() {
        let alloc = SubnetAllocator::new("172.20.0.0/16").unwrap();
        assert_eq!(alloc.base_first, 172);
        assert_eq!(alloc.base_second, 20);
    }

    #[test]
    fn new_invalid_cidr() {
        assert!(SubnetAllocator::new("invalid").is_none());
        assert!(SubnetAllocator::new("172.20.0.0/24").is_none());
        assert!(SubnetAllocator::new("not.an.ip/16").is_none());
    }

    #[test]
    fn allocate_sequential_subnets() {
        let alloc = SubnetAllocator::new("172.20.0.0/16").unwrap();
        assert_eq!(alloc.allocate().unwrap(), "172.20.1.0/24");
        assert_eq!(alloc.allocate().unwrap(), "172.20.2.0/24");
        assert_eq!(alloc.allocate().unwrap(), "172.20.3.0/24");
    }

    #[test]
    fn allocate_skips_zero_subnet() {
        let alloc = SubnetAllocator::new("172.20.0.0/16").unwrap();
        let first = alloc.allocate().unwrap();
        assert!(first.contains(".1."));
    }

    #[test]
    fn master_gateway() {
        let alloc = SubnetAllocator::new("172.20.0.0/16").unwrap();
        assert_eq!(alloc.master_gateway(), Ipv4Addr::new(172, 20, 0, 1));
    }

    #[test]
    fn master_subnet() {
        let alloc = SubnetAllocator::new("172.20.0.0/16").unwrap();
        assert_eq!(alloc.master_subnet(), "172.20.0.0/24");
    }

    #[test]
    fn different_base_cidr() {
        let alloc = SubnetAllocator::new("10.99.0.0/16").unwrap();
        assert_eq!(alloc.allocate().unwrap(), "10.99.1.0/24");
        assert_eq!(alloc.master_gateway(), Ipv4Addr::new(10, 99, 0, 1));
    }
}
