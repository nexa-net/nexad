use std::any::Any;

use nexa_core::ports::metrics::MetricsPort;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, Opts, Registry, TextEncoder,
};

pub struct PrometheusMetrics {
    registry: Registry,
    http_requests_total: IntCounterVec,
    http_request_duration: HistogramVec,
    container_events_total: IntCounterVec,
    schedule_duration: HistogramVec,
    deployment_ops_total: IntCounterVec,
    nodes_total: IntGauge,
    pods_total: IntGauge,
    deployments_total: IntGauge,
    proxy_requests_total: IntCounterVec,
    proxy_request_duration: HistogramVec,
    proxy_errors_total: IntCounterVec,
}

impl PrometheusMetrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let http_requests_total = IntCounterVec::new(
            Opts::new("nexa_http_requests_total", "Total HTTP requests"),
            &["method", "path", "status"],
        )
        .unwrap();

        let http_request_duration = HistogramVec::new(
            HistogramOpts::new(
                "nexa_http_request_duration_seconds",
                "HTTP request duration in seconds",
            ),
            &["method", "path"],
        )
        .unwrap();

        let container_events_total = IntCounterVec::new(
            Opts::new("nexa_container_events_total", "Total container lifecycle events"),
            &["event"],
        )
        .unwrap();

        let schedule_duration = HistogramVec::new(
            HistogramOpts::new(
                "nexa_schedule_duration_seconds",
                "Scheduler decision duration in seconds",
            ),
            &["strategy"],
        )
        .unwrap();

        let deployment_ops_total = IntCounterVec::new(
            Opts::new("nexa_deployment_ops_total", "Total deployment operations"),
            &["op"],
        )
        .unwrap();

        let nodes_total =
            IntGauge::new("nexa_nodes_total", "Current number of cluster nodes").unwrap();
        let pods_total = IntGauge::new("nexa_pods_total", "Current number of pods").unwrap();
        let deployments_total =
            IntGauge::new("nexa_deployments_total", "Current number of deployments").unwrap();

        let proxy_requests_total = IntCounterVec::new(
            Opts::new("nexa_proxy_requests_total", "Total proxy requests"),
            &["domain", "status"],
        )
        .unwrap();

        let proxy_request_duration = HistogramVec::new(
            HistogramOpts::new(
                "nexa_proxy_request_duration_seconds",
                "Proxy upstream request duration in seconds",
            ),
            &["domain"],
        )
        .unwrap();

        let proxy_errors_total = IntCounterVec::new(
            Opts::new("nexa_proxy_errors_total", "Total proxy errors"),
            &["domain", "error_type"],
        )
        .unwrap();

        registry.register(Box::new(http_requests_total.clone())).unwrap();
        registry.register(Box::new(http_request_duration.clone())).unwrap();
        registry.register(Box::new(container_events_total.clone())).unwrap();
        registry.register(Box::new(schedule_duration.clone())).unwrap();
        registry.register(Box::new(deployment_ops_total.clone())).unwrap();
        registry.register(Box::new(nodes_total.clone())).unwrap();
        registry.register(Box::new(pods_total.clone())).unwrap();
        registry.register(Box::new(deployments_total.clone())).unwrap();
        registry.register(Box::new(proxy_requests_total.clone())).unwrap();
        registry.register(Box::new(proxy_request_duration.clone())).unwrap();
        registry.register(Box::new(proxy_errors_total.clone())).unwrap();

        Self {
            registry,
            http_requests_total,
            http_request_duration,
            container_events_total,
            schedule_duration,
            deployment_ops_total,
            nodes_total,
            pods_total,
            deployments_total,
            proxy_requests_total,
            proxy_request_duration,
            proxy_errors_total,
        }
    }

    pub fn encode(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}

impl MetricsPort for PrometheusMetrics {
    fn record_http_request(&self, method: &str, path: &str, status: u16, duration_secs: f64) {
        self.http_requests_total
            .with_label_values(&[method, path, &status.to_string()])
            .inc();
        self.http_request_duration
            .with_label_values(&[method, path])
            .observe(duration_secs);
    }

    fn record_container_event(&self, event: &str) {
        self.container_events_total.with_label_values(&[event]).inc();
    }

    fn record_schedule_decision(&self, strategy: &str, duration_secs: f64) {
        self.schedule_duration
            .with_label_values(&[strategy])
            .observe(duration_secs);
    }

    fn record_deployment_op(&self, op: &str) {
        self.deployment_ops_total.with_label_values(&[op]).inc();
    }

    fn set_node_count(&self, count: usize) {
        self.nodes_total.set(count as i64);
    }

    fn set_pod_count(&self, count: usize) {
        self.pods_total.set(count as i64);
    }

    fn set_deployment_count(&self, count: usize) {
        self.deployments_total.set(count as i64);
    }

    fn record_proxy_request(&self, domain: &str, status: u16, duration_secs: f64) {
        self.proxy_requests_total
            .with_label_values(&[domain, &status.to_string()])
            .inc();
        self.proxy_request_duration
            .with_label_values(&[domain])
            .observe(duration_secs);
    }

    fn record_proxy_error(&self, domain: &str, error_type: &str) {
        self.proxy_errors_total
            .with_label_values(&[domain, error_type])
            .inc();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_registry_with_all_metrics() {
        let m = PrometheusMetrics::new();
        let output = m.encode();
        // Label-less IntGauges are emitted immediately (initialized to 0).
        // IntCounterVec / HistogramVec only appear after the first observation.
        assert!(output.contains("nexa_nodes_total 0"));
        assert!(output.contains("nexa_pods_total 0"));
        assert!(output.contains("nexa_deployments_total 0"));
    }

    #[test]
    fn record_http_request_appears_in_output() {
        let m = PrometheusMetrics::new();
        m.record_http_request("GET", "/health", 200, 0.001);
        let output = m.encode();
        assert!(output.contains("nexa_http_requests_total"));
        assert!(output.contains("nexa_http_request_duration_seconds"));
    }

    #[test]
    fn record_container_event_appears_in_output() {
        let m = PrometheusMetrics::new();
        m.record_container_event("died");
        let output = m.encode();
        assert!(output.contains("nexa_container_events_total"));
        assert!(output.contains("died"));
    }

    #[test]
    fn gauges_update_correctly() {
        let m = PrometheusMetrics::new();
        m.set_node_count(3);
        m.set_pod_count(10);
        m.set_deployment_count(5);
        let output = m.encode();
        assert!(output.contains("nexa_nodes_total 3"));
        assert!(output.contains("nexa_pods_total 10"));
        assert!(output.contains("nexa_deployments_total 5"));
    }

    #[test]
    fn record_deployment_op_appears_in_output() {
        let m = PrometheusMetrics::new();
        m.record_deployment_op("deploy");
        m.record_deployment_op("scale");
        let output = m.encode();
        assert!(output.contains("nexa_deployment_ops_total"));
        assert!(output.contains("deploy"));
        assert!(output.contains("scale"));
    }

    #[test]
    fn record_proxy_metrics_appear_in_output() {
        let m = PrometheusMetrics::new();
        m.record_proxy_request("api.example.com", 200, 0.05);
        m.record_proxy_error("api.example.com", "connection_refused");
        let output = m.encode();
        assert!(output.contains("nexa_proxy_requests_total"));
        assert!(output.contains("nexa_proxy_errors_total"));
        assert!(output.contains("api.example.com"));
    }

    #[test]
    fn implements_metrics_port_trait() {
        let m = PrometheusMetrics::new();
        let port: &dyn MetricsPort = &m;
        port.record_http_request("POST", "/api/v1/deploy", 201, 0.5);
        port.record_schedule_decision("spread", 0.002);
    }
}
