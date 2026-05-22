mod hickory;
mod noop;
pub mod record_store;

pub use hickory::HickoryDnsProvider;
pub use noop::NoopDnsProvider;
pub use record_store::DnsRecordStore;
