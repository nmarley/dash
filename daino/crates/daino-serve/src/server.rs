//! HTTP server setup and routing.

use std::sync::Arc;

use anyhow::Result;
use axum::routing::get;
use axum::Router;
use tower_http::cors::CorsLayer;

use daino_state::db::DainoDB;

use crate::api::{self, AppState};

/// Start the REST API server.
///
/// Opens the database at `dbdir` and listens on the given address.
pub async fn start_server(dbdir: &std::path::Path, listen_addr: &str) -> Result<()> {
    let db = DainoDB::open(dbdir)?;

    let state = Arc::new(AppState { db });

    let app = Router::new()
        .route("/api/status", get(api::get_status))
        .route("/api/block/{hash}", get(api::get_block_by_hash))
        .route("/api/block-index/{height}", get(api::get_block_by_height))
        .route("/api/tx/{txid}", get(api::get_tx))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    println!("Daino API server listening on http://{}", listen_addr);

    axum::serve(listener, app).await?;

    Ok(())
}
