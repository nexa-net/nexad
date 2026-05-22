mod api;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

use nexa_core::domain::models::*;
use nexa_core::domain::orchestrator::Orchestrator;
use nexa_core::ports::cluster::ClusterTransport;
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

/// Initialise the data directory, SQLite state store, and Docker runtime.
/// Returns (data_dir, state, runtime).
async fn init_infrastructure(
    cli: &Cli,
) -> anyhow::Result<(PathBuf, Arc<dyn StateStore>, Arc<dyn ContainerRuntime>)> {
    std::fs::create_dir_all(&cli.data_dir)?;

    let data_dir = PathBuf::from(&cli.data_dir);

    let db_path = format!("{}/nexa.db", cli.data_dir);
    let database_url = format!("sqlite:{}?mode=rwc", db_path);
    let store = nexad::adapters::state::SqliteStore::connect(&database_url).await?;
    let store: Arc<dyn StateStore> = Arc::new(store);
    info!(path = db_path, "state store initialized");

    let runtime = nexad::adapters::runtime::DockerRuntime::new()?;
    runtime.ping().await?;
    info!("connected to Docker runtime");

    let runtime: Arc<dyn ContainerRuntime> = Arc::new(runtime);
    Ok((data_dir, store, runtime))
}

/// Load or generate the master encryption key and create the encrypted secret
/// store.
fn init_secrets(cli: &Cli, data_dir: &PathBuf) -> anyhow::Result<Arc<dyn SecretStore>> {
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

/// Spawn the orchestrator together with its health checker and event watcher.
fn spawn_orchestrator(
    runtime: &Arc<dyn ContainerRuntime>,
    store: &Arc<dyn StateStore>,
    secret_store: Arc<dyn SecretStore>,
) -> nexa_core::domain::orchestrator::OrchestratorHandle {
    let transport: Arc<dyn ClusterTransport> =
        Arc::new(nexad::adapters::transport::LocalTransport::new(Arc::clone(runtime)));
    let handle = Orchestrator::spawn(
        Arc::clone(runtime),
        Some(Arc::clone(store)),
        Some(secret_store),
        Some(transport),
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

// ────────────────────── single-node mode ──────────────────────

async fn start_single_node(cli: &Cli) -> anyhow::Result<()> {
    info!("starting nexad in single-node mode on {}:{}", cli.host, cli.port);

    let (data_dir, store, runtime) = init_infrastructure(cli).await?;
    let secret_store = init_secrets(cli, &data_dir)?;
    let handle = spawn_orchestrator(&runtime, &store, secret_store);

    let addr = format!("{}:{}", cli.host, cli.port);
    api::serve(handle, &addr).await
}

// ────────────────────── master mode ──────────────────────

async fn start_master(cli: &Cli) -> anyhow::Result<()> {
    info!(
        "starting nexad in master mode on {}:{} (gRPC {})",
        cli.host, cli.port, cli.grpc_port
    );

    let (data_dir, store, runtime) = init_infrastructure(cli).await?;
    let secret_store = init_secrets(cli, &data_dir)?;
    let handle = spawn_orchestrator(&runtime, &store, secret_store);

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
            store
                .set_cluster_config("join_token_hash", &hash)
                .await?;
            info!("join token generated — workers can join with:");
            info!("  nexad --mode worker --join {}:{} --token {}", cli.host, cli.grpc_port, token);
            hash
        }
    };

    // Start gRPC server as background task.
    let grpc_addr = format!("{}:{}", cli.host, cli.grpc_port);
    let grpc_runtime = Arc::clone(&runtime);
    let grpc_state = Arc::clone(&store);
    let grpc_token_hash = token_hash.clone();
    tokio::spawn(async move {
        if let Err(e) =
            nexad::cluster::server::start_grpc_server(&grpc_addr, grpc_runtime, grpc_state, grpc_token_hash)
                .await
        {
            tracing::error!(error = %e, "gRPC cluster server failed");
        }
    });

    // Start heartbeat monitor as background task.
    let hb_state = Arc::clone(&store);
    let reschedule: nexad::cluster::heartbeat::RescheduleFn =
        Arc::new(move |node_id, pods| {
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
    api::serve(handle, &addr).await
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

    // Worker gets its own local state store and runtime.
    let db_path = format!("{}/nexa.db", cli.data_dir);
    let database_url = format!("sqlite:{}?mode=rwc", db_path);
    let store = nexad::adapters::state::SqliteStore::connect(&database_url).await?;
    let store: Arc<dyn StateStore> = Arc::new(store);
    info!(path = db_path, "worker state store initialized");

    let runtime = nexad::adapters::runtime::DockerRuntime::new()?;
    runtime.ping().await?;
    info!("connected to Docker runtime");
    let runtime: Arc<dyn ContainerRuntime> = Arc::new(runtime);

    let listen_addr = format!("{}:{}", cli.host, cli.grpc_port);

    nexad::cluster::worker::start_worker(master_addr, token, listen_addr, runtime, store).await
}
