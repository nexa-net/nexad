pub mod proto {
    tonic::include_proto!("nexa.cluster");
}

pub mod heartbeat;
pub mod server;
pub mod token;
pub mod worker;
