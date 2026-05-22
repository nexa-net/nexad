mod detect;
mod docker;

pub use detect::{RuntimeDetector, RuntimeKind};
pub use docker::DockerRuntime;
