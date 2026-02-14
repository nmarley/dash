//! HTTP server setup and routing.

use std::sync::Arc;

use anyhow::Result;
use axum::routing::get;
use axum::Router;
use tower_http::cors::CorsLayer;

use daino_fetch::rpc::DashdRpc;
use daino_state::db::DainoDB;

use crate::api::{self, AppState};

/// Optional dashd RPC configuration for live queries.
pub struct RpcConfig {
    pub url: String,
    pub user: String,
    pub password: String,
}

/// Start the REST API server.
///
/// Opens the database at `dbdir` and listens on the given address.
/// If `rpc_config` is provided, dashd-backed endpoints will be available.
pub async fn start_server(
    dbdir: &std::path::Path,
    listen_addr: &str,
    rpc_config: Option<RpcConfig>,
) -> Result<()> {
    let db = DainoDB::open(dbdir)?;

    let rpc = if let Some(config) = rpc_config {
        let rpc = DashdRpc::new(&config.url, &config.user, &config.password);
        // Verify connectivity
        match rpc.ping().await {
            Ok(()) => {
                println!("Connected to dashd at {}", config.url);
                Some(rpc)
            }
            Err(e) => {
                eprintln!(
                    "WARNING: Could not connect to dashd at {}: {}",
                    config.url, e
                );
                eprintln!("         Live endpoints (chainlock, sporks, governance) will be unavailable.");
                None
            }
        }
    } else {
        println!("No dashd RPC configured. Live endpoints will be unavailable.");
        None
    };

    let state = Arc::new(AppState { db, rpc });

    let app = Router::new()
        // Index-backed endpoints (always available)
        .route("/api/status", get(api::get_status))
        .route("/api/block/{hash}", get(api::get_block_by_hash))
        .route("/api/block-index/{height}", get(api::get_block_by_height))
        .route("/api/tx/{txid}", get(api::get_tx))
        .route("/api/addr/{addr}", get(api::get_addr_summary))
        .route("/api/addr/{addr}/txs", get(api::get_addr_txs))
        .route("/api/addr/{addr}/utxo", get(api::get_addr_utxos))
        // Dashd-backed endpoints (require RPC connection)
        .route("/api/chainlock", get(api::get_chainlock))
        .route("/api/sporks", get(api::get_sporks))
        .route("/api/governance/list", get(api::get_governance_list))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    println!("Daino API server listening on http://{}", listen_addr);

    axum::serve(listener, app).await?;

    Ok(())
}
