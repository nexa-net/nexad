use std::net::{IpAddr, Ipv4Addr};
use std::time::Instant;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

use nexad::adapters::dns::DnsRecordStore;

fn ip(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(a, b, c, d))
}

fn bench_dns_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("dns_lookup");

    for &n in &[10usize, 100usize, 1000usize] {
        group.bench_with_input(BenchmarkId::new("records", n), &n, |b, &count| {
            // Pre-populate store with `count` different deployments, one IP each
            let store = DnsRecordStore::new();
            for i in 0..count {
                let deployment = format!("svc-{i}");
                // Simple IP derivation that fits in u8 ranges
                let a = ((i >> 16) & 0xFF) as u8;
                let b = ((i >> 8) & 0xFF) as u8;
                let c2 = (i & 0xFF) as u8;
                store.register("bench", &deployment, ip(10, a, b, c2));
            }

            b.iter_custom(|iters| {
                let start = Instant::now();
                for iter in 0..iters {
                    // Rotate through deployed services
                    let idx = (iter as usize) % count;
                    let query = format!("svc-{idx}.bench.internal");
                    let result = store.resolve(&query);
                    assert!(result.is_some(), "expected Some for {query}");
                }
                start.elapsed()
            });
        });
    }

    group.finish();
}

fn bench_dns_register_deregister(c: &mut Criterion) {
    c.bench_function("dns_register_deregister", |b| {
        let store = DnsRecordStore::new();

        b.iter_custom(|iters| {
            let start = Instant::now();
            for i in 0..iters {
                let addr = ip(10, 0, ((i >> 8) & 0xFF) as u8, (i & 0xFF) as u8);
                store.register("bench", "api", addr);
                store.deregister("bench", "api", addr);
            }
            start.elapsed()
        });
    });
}

criterion_group!(benches, bench_dns_lookup, bench_dns_register_deregister);
criterion_main!(benches);
