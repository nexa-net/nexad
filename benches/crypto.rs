use std::time::Instant;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use nexa_core::ports::secrets::SecretStore;
use rusqlite::Connection;
use tokio::runtime::Runtime;

use nexad::adapters::secrets::EncryptedSqliteSecretStore;

fn make_rt() -> Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

fn make_store() -> EncryptedSqliteSecretStore {
    let conn = Connection::open_in_memory().unwrap();
    let master_key = [0xABu8; 32];
    EncryptedSqliteSecretStore::new(conn, &master_key).unwrap()
}

fn bench_encrypt(c: &mut Criterion) {
    let rt = make_rt();
    let mut group = c.benchmark_group("encrypt");

    let sizes: &[(&str, usize)] = &[("64B", 64), ("1KB", 1024), ("64KB", 65536)];

    for &(label, size) in sizes {
        let payload: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter_custom(|iters| {
                rt.block_on(async {
                    let store = make_store();
                    let start = Instant::now();
                    for i in 0..iters {
                        let name = format!("secret-{i}");
                        store.set("bench-project", &name, &payload).await.unwrap();
                    }
                    start.elapsed()
                })
            });
        });
    }

    group.finish();
}

fn bench_decrypt(c: &mut Criterion) {
    let rt = make_rt();
    let mut group = c.benchmark_group("decrypt");

    let sizes: &[(&str, usize)] = &[("64B", 64), ("1KB", 1024)];

    for &(label, size) in sizes {
        let payload: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter_custom(|iters| {
                rt.block_on(async {
                    let store = make_store();

                    // Pre-populate one secret to read back
                    store.set("bench-project", "bench-key", &payload).await.unwrap();

                    let start = Instant::now();
                    for _ in 0..iters {
                        let val = store.get("bench-project", "bench-key").await.unwrap();
                        assert!(val.is_some());
                    }
                    start.elapsed()
                })
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_encrypt, bench_decrypt);
criterion_main!(benches);
