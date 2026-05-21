mod api;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

use nexa_core::domain::orchestrator::Orchestrator;
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    info!("starting nexad on {}:{}", cli.host, cli.port);

    std::fs::create_dir_all(&cli.data_dir)?;

    let data_dir = PathBuf::from(&cli.data_dir);

    let db_path = format!("{}/nexa.db", cli.data_dir);
    let database_url = format!("sqlite:{}?mode=rwc", db_path);
    let store = nexad::adapters::state::SqliteStore::connect(&database_url).await?;
    let store: Arc<dyn StateStore> = Arc::new(store);
    info!(path = db_path, "state store initialized");

    // Load or generate master encryption key
    let master_key = nexad::crypto::master_key::load_or_generate(&data_dir)?;
    info!("master key loaded");

    // Create encrypted secret store
    let secret_conn = rusqlite::Connection::open(format!("{}/secrets.db", cli.data_dir))
        .map_err(|e| anyhow::anyhow!("failed to open secrets db: {e}"))?;
    let secret_store: Arc<dyn SecretStore> = Arc::new(
        nexad::adapters::secrets::EncryptedSqliteSecretStore::new(secret_conn, &master_key)?,
    );
    info!("secret store initialized");

    let runtime = nexad::adapters::runtime::DockerRuntime::new()?;
    runtime.ping().await?;
    info!("connected to Docker runtime");

    let runtime: Arc<dyn nexa_core::ports::runtime::ContainerRuntime> = Arc::new(runtime);
    let handle = Orchestrator::spawn(Arc::clone(&runtime), Some(store), Some(secret_store));

    // Spawn health checker background task
    let health_checker = Arc::new(nexad::adapters::health::HealthChecker::new(handle.clone()));
    tokio::spawn(async move { health_checker.run().await });
    info!("health checker started");

    // Start container event watcher
    nexad::adapters::event_watcher::spawn_event_watcher(
        Arc::clone(&runtime),
        handle.command_sender(),
    );
    info!("container event watcher started");

    let addr = format!("{}:{}", cli.host, cli.port);
    api::serve(handle, &addr).await
}
