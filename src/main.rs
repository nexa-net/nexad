use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

use nexa_core::domain::models::*;
use nexa_core::domain::orchestrator::Orchestrator;
use nexa_core::ports::cluster::ClusterTransport;
use nexa_core::ports::dns::DnsProvider;
use nexa_core::ports::metrics::MetricsPort;
use nexa_core::ports::runtime::ContainerRuntime;
use nexa_core::ports::secrets::SecretStore;
use nexa_core::ports::state::StateStore;

#[derive(Parser)]
#[command(name = "nexad", about = "NexaNet daemon", version)]
struct Cli {
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    #[arg(long, default_value = "6443")]
    port: u16,

    #[arg(long, default_value = "/var/lib/nexa")]
    data_dir: String,

    /// Node mode: single, master, or worker
    #[arg(long, default_value = "single")]
    mode: String,

    /// Master address to join (worker mode only)
    #[arg(long)]
    join: Option<String>,

    /// Join token (worker mode only)
    #[arg(long)]
    token: Option<String>,

    /// gRPC listen port (master and worker modes)
    #[arg(long, default_value = "6444")]
    grpc_port: u16,

    /// DNS mode: "noop" for single-node (Docker DNS), "embedded" for multi-node
    #[arg(long, default_value = "noop")]
    dns_mode: String,

    /// IP address of this node (used for container DNS config in embedded mode)
    #[arg(long)]
    master_ip: Option<String>,

    /// DNS listen address for embedded DNS server
    #[arg(long, default_value = "0.0.0.0:15353")]
    dns_listen: String,

    /// Upstream DNS server for forwarding non-.internal queries
    #[arg(long, default_value = "8.8.8.8:53")]
    dns_upstream: String,

    /// Proxy backend: "nexa-proxy", "nginx", "caddy", "traefik"
    #[arg(long, default_value = "nexa-proxy")]
    proxy_backend: String,

    /// Proxy config directory
    #[arg(long, default_value = "/var/lib/nexa/proxy")]
    proxy_config_dir: String,

    /// ACME email for automatic TLS
    #[arg(long)]
    acme_email: Option<String>,

    /// Cluster CIDR for overlay network
    #[arg(long, default_value = "172.20.0.0/16")]
    cluster_cidr: String,

    /// WireGuard listen port
    #[arg(long, default_value = "51820")]
    wg_port: u16,

    /// Enable overlay network
    #[arg(long)]
    overlay: bool,

    /// Container runtime to use: docker, containerd, or auto
    #[arg(long, default_value = "auto")]
    runtime: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.mode.as_str() {
        "single" => start_single_node(&cli).await,
        "master" => start_master(&cli).await,
        "worker" => start_worker(&cli).await,
        other => anyhow::bail!("unknown mode: {other}. Use: single, master, or worker"),
    }
}

// ────────────────────── shared helpers ──────────────────────

/// Initialise the data directory, SQLite state store, and container runtime.
/// Returns (data_dir, state, runtime).
async fn init_infrastructure(
    cli: &Cli,
) -> anyhow::Result<(PathBuf, Arc<dyn StateStore>, Arc<dyn ContainerRuntime>)> {
    use nexad::adapters::runtime::{RuntimeDetector, RuntimeKind};

    std::fs::create_dir_all(&cli.data_dir)?;

    let data_dir = PathBuf::from(&cli.data_dir);

    let db_path = format!("{}/nexa.db", cli.data_dir);
    let database_url = format!("sqlite:{}?mode=rwc", db_path);
    let store = nexad::adapters::state::SqliteStore::connect(&database_url).await?;
    let store: Arc<dyn StateStore> = Arc::new(store);
    info!(path = db_path, "state store initialized");

    let kind: RuntimeKind = cli
        .runtime
        .parse()
        .map_err(|e: String| anyhow::anyhow!(e))?;
    let resolved = RuntimeDetector::resolve(kind).unwrap_or(RuntimeKind::Docker);
    let runtime = RuntimeDetector::build(resolved, &cli.data_dir).await?;
    info!(
        runtime = runtime.runtime_name(),
        "container runtime initialized"
    );

    Ok((data_dir, store, runtime))
}

/// Load or generate the master encryption key and create the encrypted secret
/// store.
fn init_secrets(cli: &Cli, data_dir: &Path) -> anyhow::Result<Arc<dyn SecretStore>> {
    let master_key = nexad::crypto::master_key::load_or_generate(data_dir)?;
    info!("master key loaded");

    let secret_conn = rusqlite::Connection::open(format!("{}/secrets.db", cli.data_dir))
        .map_err(|e| anyhow::anyhow!("failed to open secrets db: {e}"))?;
    let secret_store: Arc<dyn SecretStore> = Arc::new(
        nexad::adapters::secrets::EncryptedSqliteSecretStore::new(secret_conn, &master_key)?,
    );
    info!("secret store initialized");

    Ok(secret_store)
}

/// Initialise the proxy backend and in-memory route store.
fn init_proxy(
    cli: &Cli,
) -> anyhow::Result<(
    Arc<dyn nexa_core::ports::proxy::ProxyBackend>,
    Arc<dyn nexa_core::ports::route_store::RouteStore>,
)> {
    use nexad::adapters::proxy::{CaddyBackend, NexaProxyBackend, NginxBackend, TraefikBackend};
    use nexad::adapters::state::memory_route_store::InMemoryRouteStore;

    std::fs::create_dir_all(&cli.proxy_config_dir)?;

    let proxy: Arc<dyn nexa_core::ports::proxy::ProxyBackend> = match cli.proxy_backend.as_str() {
        "nginx" => Arc::new(NginxBackend::new(
            PathBuf::from(&cli.proxy_config_dir),
            "nginx".into(),
        )),
        "caddy" => {
            let caddyfile = PathBuf::from(&cli.proxy_config_dir).join("Caddyfile");
            Arc::new(CaddyBackend::new(caddyfile, "http://localhost:2019".into()))
        }
        "traefik" => {
            let config_path = PathBuf::from(&cli.proxy_config_dir).join("nexa-dynamic.yml");
            Arc::new(TraefikBackend::new(config_path))
        }
        _ => {
            let config_path = PathBuf::from(&cli.proxy_config_dir).join("proxy.json");
            Arc::new(NexaProxyBackend::new(
                config_path,
                "nexa-proxy",
                "0.0.0.0:80",
                Some("0.0.0.0:443".into()),
            ))
        }
    };

    let route_store: Arc<dyn nexa_core::ports::route_store::RouteStore> =
        Arc::new(InMemoryRouteStore::new());

    info!(backend = %cli.proxy_backend, "proxy backend initialized");
    Ok((proxy, route_store))
}

/// Spawn the orchestrator together with its health checker and event watcher.
fn spawn_orchestrator(
    runtime: &Arc<dyn ContainerRuntime>,
    store: &Arc<dyn StateStore>,
    secret_store: Arc<dyn SecretStore>,
    dns: Option<Arc<dyn DnsProvider>>,
    master_ip: Option<String>,
    proxy: Option<Arc<dyn nexa_core::ports::proxy::ProxyBackend>>,
    route_store: Option<Arc<dyn nexa_core::ports::route_store::RouteStore>>,
) -> nexa_core::domain::orchestrator::OrchestratorHandle {
    let transport: Arc<dyn ClusterTransport> = Arc::new(
        nexad::adapters::transport::LocalTransport::new(Arc::clone(runtime)),
    );
    let handle = Orchestrator::spawn(
        Arc::clone(runtime),
        Some(Arc::clone(store)),
        Some(secret_store),
        Some(transport),
        dns,
        master_ip,
        proxy,
        route_store,
        None,
    );

    // Spawn health checker background task
    let health_checker = Arc::new(nexad::adapters::health::HealthChecker::new(handle.clone()));
    tokio::spawn(async move { health_checker.run().await });
    info!("health checker started");

    // Start container event watcher
    nexad::adapters::event_watcher::spawn_event_watcher(
        Arc::clone(runtime),
        handle.command_sender(),
    );
    info!("container event watcher started");

    handle
}

/// Initialise the DNS provider based on --dns-mode CLI flag.
async fn init_dns(cli: &Cli) -> anyhow::Result<(Option<Arc<dyn DnsProvider>>, Option<String>)> {
    match cli.dns_mode.as_str() {
        "embedded" => {
            let listen_addr: std::net::SocketAddr = cli
                .dns_listen
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid --dns-listen address: {e}"))?;
            let upstream_addr: std::net::SocketAddr = cli
                .dns_upstream
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid --dns-upstream address: {e}"))?;

            let provider =
                nexad::adapters::dns::HickoryDnsProvider::new(listen_addr, upstream_addr);
            provider.start().await?;
            info!(listen = %cli.dns_listen, upstream = %cli.dns_upstream, "embedded DNS server started");

            let master_ip = cli.master_ip.clone();
            Ok((Some(Arc::new(provider) as Arc<dyn DnsProvider>), master_ip))
        }
        _ => {
            info!("using noop DNS (single-node, containers use Docker DNS)");
            Ok((None, None))
        }
    }
}

// ────────────────────── single-node mode ──────────────────────

async fn start_single_node(cli: &Cli) -> anyhow::Result<()> {
    info!(
        "starting nexad in single-node mode on {}:{}",
        cli.host, cli.port
    );

    let (data_dir, store, runtime) = init_infrastructure(cli).await?;
    let secret_store = init_secrets(cli, &data_dir)?;
    let (dns, master_ip) = init_dns(cli).await?;
    let (proxy, route_store) = init_proxy(cli)?;
    let handle = spawn_orchestrator(
        &runtime,
        &store,
        secret_store,
        dns,
        master_ip,
        Some(Arc::clone(&proxy)),
        Some(Arc::clone(&route_store)),
    );

    if let Some(ref email) = cli.acme_email {
        let acme = Arc::new(nexad::adapters::tls::AcmeManager::new(
            email,
            Arc::clone(&route_store),
            false,
        ));
        nexad::adapters::tls::spawn_renewal_task(
            Arc::clone(&route_store),
            acme,
            std::time::Duration::from_secs(86400),
            30,
        );
        info!(email, "TLS auto-renewal enabled");
    }

    let addr = format!("{}:{}", cli.host, cli.port);
    let noop_metrics: Arc<dyn MetricsPort> = Arc::new(nexa_core::ports::metrics::NoOpMetrics);
    nexad::api::serve(handle, Arc::clone(&store), noop_metrics, &addr).await
}

// ────────────────────── master mode ──────────────────────

async fn start_master(cli: &Cli) -> anyhow::Result<()> {
    info!(
        "starting nexad in master mode on {}:{} (gRPC {})",
        cli.host, cli.port, cli.grpc_port
    );

    let (data_dir, store, runtime) = init_infrastructure(cli).await?;
    let secret_store = init_secrets(cli, &data_dir)?;
    let (dns, master_ip) = init_dns(cli).await?;
    let (proxy, route_store) = init_proxy(cli)?;
    let handle = spawn_orchestrator(
        &runtime,
        &store,
        secret_store,
        dns,
        master_ip,
        Some(Arc::clone(&proxy)),
        Some(Arc::clone(&route_store)),
    );

    if let Some(ref email) = cli.acme_email {
        let acme = Arc::new(nexad::adapters::tls::AcmeManager::new(
            email,
            Arc::clone(&route_store),
            false,
        ));
        nexad::adapters::tls::spawn_renewal_task(
            Arc::clone(&route_store),
            acme,
            std::time::Duration::from_secs(86400),
            30,
        );
        info!(email, "TLS auto-renewal enabled");
    }

    // Register self as a master node.
    let hostname = hostname::get()
        .map_err(|e| anyhow::anyhow!("failed to get hostname: {e}"))?
        .to_string_lossy()
        .to_string();
    let resources = nexad::cluster::heartbeat::collect_resources();
    let master_node = Node::new(
        hostname.clone(),
        format!("{}:{}", cli.host, cli.grpc_port),
        NodeRole::Master,
        resources,
    );
    let _ = store.insert_node(&master_node).await;
    info!(node_id = %master_node.id, name = %hostname, "master node registered");

    // Generate or load the join token.
    let token_hash = match store.get_cluster_config("join_token_hash").await? {
        Some(hash) => {
            info!("loaded existing join token from cluster config");
            hash
        }
        None => {
            let token = nexad::cluster::token::generate_token();
            let hash = nexad::cluster::token::hash_token(&token);
            store.set_cluster_config("join_token_hash", &hash).await?;
            info!("join token generated — workers can join with:");
            info!(
                "  nexad --mode worker --join {}:{} --token {}",
                cli.host, cli.grpc_port, token
            );
            hash
        }
    };

    // Start gRPC server as background task.
    let grpc_addr = format!("{}:{}", cli.host, cli.grpc_port);
    let grpc_runtime = Arc::clone(&runtime);
    let grpc_state = Arc::clone(&store);
    let grpc_token_hash = token_hash.clone();
    tokio::spawn(async move {
        if let Err(e) = nexad::cluster::server::start_grpc_server(
            &grpc_addr,
            grpc_runtime,
            grpc_state,
            grpc_token_hash,
        )
        .await
        {
            tracing::error!(error = %e, "gRPC cluster server failed");
        }
    });

    // Start heartbeat monitor as background task.
    let hb_state = Arc::clone(&store);
    let reschedule: nexad::cluster::heartbeat::RescheduleFn = Arc::new(move |node_id, pods| {
        tracing::warn!(
            node_id = %node_id,
            pod_count = pods.len(),
            "dead node — pods need rescheduling (TODO: implement scheduler)"
        );
    });
    tokio::spawn(async move {
        nexad::cluster::heartbeat::run_monitor(hb_state, reschedule).await;
    });
    info!("heartbeat monitor started");

    // Start the HTTP API (blocks).
    let addr = format!("{}:{}", cli.host, cli.port);
    let noop_metrics: Arc<dyn MetricsPort> = Arc::new(nexa_core::ports::metrics::NoOpMetrics);
    nexad::api::serve(handle, Arc::clone(&store), noop_metrics, &addr).await
}

// ────────────────────── worker mode ──────────────────────

async fn start_worker(cli: &Cli) -> anyhow::Result<()> {
    let master_addr = cli
        .join
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("--join is required in worker mode"))?
        .to_string();
    let token = cli
        .token
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("--token is required in worker mode"))?
        .to_string();

    info!(
        "starting nexad in worker mode, joining master at {}",
        master_addr
    );

    std::fs::create_dir_all(&cli.data_dir)?;

    // Worker gets its own local state store and runtime (respects --runtime flag).
    let db_path = format!("{}/nexa.db", cli.data_dir);
    let database_url = format!("sqlite:{}?mode=rwc", db_path);
    let store = nexad::adapters::state::SqliteStore::connect(&database_url).await?;
    let store: Arc<dyn StateStore> = Arc::new(store);
    info!(path = db_path, "worker state store initialized");

    use nexad::adapters::runtime::{RuntimeDetector, RuntimeKind};
    let kind: RuntimeKind = cli
        .runtime
        .parse()
        .map_err(|e: String| anyhow::anyhow!(e))?;
    let resolved = RuntimeDetector::resolve(kind).unwrap_or(RuntimeKind::Docker);
    let runtime = RuntimeDetector::build(resolved, &cli.data_dir).await?;
    info!(
        runtime = runtime.runtime_name(),
        "worker container runtime initialized"
    );

    let listen_addr = format!("{}:{}", cli.host, cli.grpc_port);

    nexad::cluster::worker::start_worker(master_addr, token, listen_addr, runtime, store).await
}
