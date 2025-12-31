mod config;
mod grpc;

use anyhow::Result;
use clap::Parser;
use config::Config;
use dash_state::IndexDatabase;
use dash_sync::ZmqConsumer;
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(name = "dash-indexer")]
#[command(about = "Modular indexer service for Dash blockchain", long_about = None)]
struct Args {
    #[arg(short, long, default_value = "dash-indexer.toml")]
    config: PathBuf,

    #[arg(long, default_value = "./index-data")]
    datadir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    info!("Starting dash-indexer v{}", env!("CARGO_PKG_VERSION"));
    info!("Data directory: {:?}", args.datadir);

    // Open database
    let db = IndexDatabase::new(&args.datadir)?;
    info!("Database opened successfully");

    // Check sync status
    if let Some(tip) = db.get_tip_height()? {
        info!("Current tip height: {}", tip);
    } else {
        info!("Database is empty, starting fresh sync");
    }

    // TODO: Load configuration
    // TODO: Start ZMQ consumer
    // TODO: Start gRPC server
    // TODO: Start JSON-RPC server (optional)

    info!("Indexer is running (placeholder - press Ctrl+C to exit)");

    // Keep running
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");

    Ok(())
}
