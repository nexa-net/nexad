use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

use nexa_core::domain::orchestrator::Command;
use nexa_core::ports::runtime::{ContainerRuntime, RuntimeEvent};

pub fn spawn_event_watcher(
    runtime: Arc<dyn ContainerRuntime>,
    tx: mpsc::Sender<Command>,
) {
    tokio::spawn(async move {
        info!("container event watcher starting");
        loop {
            match runtime.events().await {
                Ok(stream) => {
                    handle_event_stream(stream, &tx).await;
                    warn!("event stream ended, reconnecting in 5s");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                Err(e) => {
                    error!(error = %e, "failed to open event stream, retrying in 5s");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    });
}

async fn handle_event_stream(
    mut stream: nexa_core::ports::runtime::EventStream,
    tx: &mpsc::Sender<Command>,
) {
    while let Some(event) = stream.next().await {
        match event {
            RuntimeEvent::ContainerDied { container_id, exit_code } => {
                info!(container_id, exit_code, "container died event");
                if let Some(pod_id) = extract_pod_id(&container_id) {
                    let cmd = Command::ContainerExited { pod_id, exit_code };
                    if tx.send(cmd).await.is_err() {
                        error!("orchestrator channel closed, stopping event watcher");
                        return;
                    }
                }
            }
            RuntimeEvent::ContainerOom { container_id } => {
                warn!(container_id, "container OOM event");
                if let Some(pod_id) = extract_pod_id(&container_id) {
                    let cmd = Command::ContainerExited { pod_id, exit_code: 137 };
                    if tx.send(cmd).await.is_err() {
                        error!("orchestrator channel closed, stopping event watcher");
                        return;
                    }
                }
            }
            RuntimeEvent::ContainerStarted { container_id } => {
                info!(container_id, "container started event (ignored)");
            }
        }
    }
}

fn extract_pod_id(container_id: &str) -> Option<Uuid> {
    Uuid::parse_str(container_id).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_pod_id_parses_valid_uuid() {
        let id = Uuid::new_v4();
        assert_eq!(extract_pod_id(&id.to_string()), Some(id));
    }

    #[test]
    fn extract_pod_id_returns_none_for_non_uuid() {
        assert_eq!(extract_pod_id("abc123def"), None);
    }

    #[tokio::test]
    async fn event_watcher_forwards_container_died() {
        let pod_id = Uuid::new_v4();
        let events = vec![RuntimeEvent::ContainerDied {
            container_id: pod_id.to_string(),
            exit_code: 1,
        }];
        let (tx, mut rx) = mpsc::channel(16);
        let stream: nexa_core::ports::runtime::EventStream =
            Box::pin(futures::stream::iter(events));
        handle_event_stream(stream, &tx).await;
        let cmd = rx.try_recv().expect("should have received a command");
        match cmd {
            Command::ContainerExited { pod_id: pid, exit_code } => {
                assert_eq!(pid, pod_id);
                assert_eq!(exit_code, 1);
            }
            _ => panic!("unexpected command variant"),
        }
    }

    #[tokio::test]
    async fn event_watcher_forwards_oom_as_exit_137() {
        let pod_id = Uuid::new_v4();
        let events = vec![RuntimeEvent::ContainerOom {
            container_id: pod_id.to_string(),
        }];
        let (tx, mut rx) = mpsc::channel(16);
        let stream: nexa_core::ports::runtime::EventStream =
            Box::pin(futures::stream::iter(events));
        handle_event_stream(stream, &tx).await;
        let cmd = rx.try_recv().expect("should have received a command");
        match cmd {
            Command::ContainerExited { pod_id: pid, exit_code } => {
                assert_eq!(pid, pod_id);
                assert_eq!(exit_code, 137);
            }
            _ => panic!("unexpected command variant"),
        }
    }

    #[tokio::test]
    async fn event_watcher_ignores_started_events() {
        let events = vec![RuntimeEvent::ContainerStarted {
            container_id: Uuid::new_v4().to_string(),
        }];
        let (tx, mut rx) = mpsc::channel(16);
        let stream: nexa_core::ports::runtime::EventStream =
            Box::pin(futures::stream::iter(events));
        handle_event_stream(stream, &tx).await;
        assert!(rx.try_recv().is_err(), "should not forward started events");
    }
}
