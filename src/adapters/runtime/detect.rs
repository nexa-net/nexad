use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use nexa_core::ports::runtime::ContainerRuntime;

use tracing::info;

use super::DockerRuntime;

/// Which container runtime to use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RuntimeKind {
    Docker,
    Containerd,
    Auto,
}

impl FromStr for RuntimeKind {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "docker" => Ok(RuntimeKind::Docker),
            "containerd" => Ok(RuntimeKind::Containerd),
            "auto" => Ok(RuntimeKind::Auto),
            other => Err(format!("unknown runtime: {other}. Use: docker, containerd, or auto")),
        }
    }
}

impl fmt::Display for RuntimeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeKind::Docker => write!(f, "docker"),
            RuntimeKind::Containerd => write!(f, "containerd"),
            RuntimeKind::Auto => write!(f, "auto"),
        }
    }
}

/// Detects and builds the appropriate container runtime.
pub struct RuntimeDetector;

impl RuntimeDetector {
    /// Probe for Docker and containerd sockets on the filesystem.
    /// Returns (docker_available, containerd_available).
    pub fn probe() -> (bool, bool) {
        let docker = std::path::Path::new("/var/run/docker.sock").exists();
        let containerd = std::path::Path::new("/run/containerd/containerd.sock").exists();
        (docker, containerd)
    }

    /// Automatically detect the best available runtime.
    /// Prefers Docker when both are present.
    pub fn auto_detect() -> anyhow::Result<RuntimeKind> {
        let (docker, containerd) = Self::probe();
        if docker {
            Ok(RuntimeKind::Docker)
        } else if containerd {
            Ok(RuntimeKind::Containerd)
        } else {
            anyhow::bail!("no container runtime found: neither Docker nor containerd socket detected")
        }
    }

    /// Resolve `Auto` to a concrete runtime kind, or pass through Docker/Containerd.
    pub fn resolve(kind: RuntimeKind) -> anyhow::Result<RuntimeKind> {
        match kind {
            RuntimeKind::Auto => Self::auto_detect(),
            other => Ok(other),
        }
    }

    /// Build a runtime instance for the given kind.
    pub async fn build(
        kind: RuntimeKind,
        data_dir: &str,
    ) -> anyhow::Result<Arc<dyn ContainerRuntime>> {
        match kind {
            RuntimeKind::Docker => {
                let rt = DockerRuntime::new()?;
                rt.ping().await?;
                info!("connected to Docker runtime");
                Ok(Arc::new(rt))
            }
            RuntimeKind::Containerd => {
                let rt = super::ContainerdRuntime::new(data_dir)?;
                rt.ping().await?;
                info!("connected to containerd runtime");
                Ok(Arc::new(rt))
            }
            RuntimeKind::Auto => {
                anyhow::bail!("RuntimeKind::Auto must be resolved before building")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_runtime_kind_docker() {
        assert_eq!("docker".parse::<RuntimeKind>().unwrap(), RuntimeKind::Docker);
    }

    #[test]
    fn parse_runtime_kind_containerd() {
        assert_eq!("containerd".parse::<RuntimeKind>().unwrap(), RuntimeKind::Containerd);
    }

    #[test]
    fn parse_runtime_kind_auto() {
        assert_eq!("auto".parse::<RuntimeKind>().unwrap(), RuntimeKind::Auto);
    }

    #[test]
    fn parse_runtime_kind_case_insensitive() {
        assert_eq!("Docker".parse::<RuntimeKind>().unwrap(), RuntimeKind::Docker);
        assert_eq!("CONTAINERD".parse::<RuntimeKind>().unwrap(), RuntimeKind::Containerd);
        assert_eq!("AUTO".parse::<RuntimeKind>().unwrap(), RuntimeKind::Auto);
    }

    #[test]
    fn parse_runtime_kind_invalid() {
        assert!("podman".parse::<RuntimeKind>().is_err());
        assert!("".parse::<RuntimeKind>().is_err());
    }

    #[test]
    fn display_runtime_kind() {
        assert_eq!(RuntimeKind::Docker.to_string(), "docker");
        assert_eq!(RuntimeKind::Containerd.to_string(), "containerd");
        assert_eq!(RuntimeKind::Auto.to_string(), "auto");
    }

    #[test]
    fn probe_returns_booleans() {
        let (docker, containerd) = RuntimeDetector::probe();
        // We just verify these are booleans and the call doesn't panic.
        let _ = docker;
        let _ = containerd;
    }
}
