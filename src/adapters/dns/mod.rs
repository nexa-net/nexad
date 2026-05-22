mod noop;
pub mod record_store;

pub use noop::NoopDnsProvider;
pub use record_store::DnsRecordStore;
