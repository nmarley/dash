//! REST API endpoint handlers.
//!
//! Modeled after the Insight API:
//! - GET /api/status           -- chain status
//! - GET /api/block/:hash      -- block by hash
//! - GET /api/block-index/:h   -- block hash by height
//! - GET /api/tx/:txid         -- transaction by txid

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::Serialize;

use daino_state::db::DainoDB;
use librustdash::hash::{hash_to_display, reverse_hash};
use librustdash::script::decode_address;

/// Shared application state passed to all handlers.
pub struct AppState {
    pub db: DainoDB,
}

// -- Response types --

#[derive(Serialize)]
pub struct StatusResponse {
    pub info: ChainInfo,
}

#[derive(Serialize)]
pub struct ChainInfo {
    pub blocks: u32,
    #[serde(rename = "bestblockhash")]
    pub best_block_hash: String,
    #[serde(rename = "txcount")]
    pub tx_count: u64,
    pub version: String,
}

#[derive(Serialize)]
pub struct BlockResponse {
    pub hash: String,
    pub height: u32,
    #[serde(rename = "previousblockhash")]
    pub previous_block_hash: String,
    #[serde(rename = "merkleroot")]
    pub merkle_root: String,
    pub time: u32,
    pub bits: String,
    pub nonce: u32,
    #[serde(rename = "txcount")]
    pub tx_count: u32,
    pub size: u32,
}

#[derive(Serialize)]
pub struct BlockHashResponse {
    #[serde(rename = "blockHash")]
    pub block_hash: String,
}

#[derive(Serialize)]
pub struct TxResponse {
    pub txid: String,
    #[serde(rename = "blockheight")]
    pub block_height: u32,
    #[serde(rename = "txindex")]
    pub tx_index: u32,
    pub version: i16,
    #[serde(rename = "type")]
    pub tx_type: u16,
    pub locktime: u32,
    #[serde(rename = "valueOut")]
    pub value_out: f64,
    #[serde(rename = "inputCount")]
    pub input_count: u32,
    #[serde(rename = "outputCount")]
    pub output_count: u32,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// -- Handlers --

/// GET /api/status
pub async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<StatusResponse>, StatusCode> {
    let meta = state
        .db
        .get_meta()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match meta {
        Some(meta) => Ok(Json(StatusResponse {
            info: ChainInfo {
                blocks: meta.tip_height,
                best_block_hash: hash_to_display(&meta.tip_hash),
                tx_count: meta.tx_count,
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        })),
        None => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

/// GET /api/block/:hash_hex
pub async fn get_block_by_hash(
    State(state): State<Arc<AppState>>,
    Path(hash_hex): Path<String>,
) -> Result<Json<BlockResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Parse the display-order hex hash back to internal byte order
    let hash_bytes = hex::decode(&hash_hex).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid hash hex".to_string(),
            }),
        )
    })?;

    if hash_bytes.len() != 32 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Hash must be 32 bytes (64 hex chars)".to_string(),
            }),
        ));
    }

    // Display order -> internal order (reverse)
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&hash_bytes);
    let internal_hash = reverse_hash(&hash);

    let block = state
        .db
        .get_block_by_hash(&internal_hash)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Database error".to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Block not found: {}", hash_hex),
                }),
            )
        })?;

    Ok(Json(block_to_response(&block)))
}

/// GET /api/block-index/:height
pub async fn get_block_by_height(
    State(state): State<Arc<AppState>>,
    Path(height): Path<u32>,
) -> Result<Json<BlockHashResponse>, (StatusCode, Json<ErrorResponse>)> {
    let block = state
        .db
        .get_block_by_height(height)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Database error".to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Block not found at height {}", height),
                }),
            )
        })?;

    Ok(Json(BlockHashResponse {
        block_hash: hash_to_display(&block.hash),
    }))
}

/// GET /api/tx/:txid_hex
pub async fn get_tx(
    State(state): State<Arc<AppState>>,
    Path(txid_hex): Path<String>,
) -> Result<Json<TxResponse>, (StatusCode, Json<ErrorResponse>)> {
    let txid_bytes = hex::decode(&txid_hex).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid txid hex".to_string(),
            }),
        )
    })?;

    if txid_bytes.len() != 32 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Txid must be 32 bytes (64 hex chars)".to_string(),
            }),
        ));
    }

    // Display order -> internal order
    let mut txid = [0u8; 32];
    txid.copy_from_slice(&txid_bytes);
    let internal_txid = reverse_hash(&txid);

    let tx = state
        .db
        .get_tx(&internal_txid)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Database error".to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Transaction not found: {}", txid_hex),
                }),
            )
        })?;

    Ok(Json(TxResponse {
        txid: hash_to_display(&tx.txid),
        block_height: tx.block_height,
        tx_index: tx.tx_index,
        version: tx.version,
        tx_type: tx.tx_type,
        locktime: tx.lock_time,
        value_out: tx.value_out as f64 / 100_000_000.0,
        input_count: tx.input_count,
        output_count: tx.output_count,
    }))
}

// -- Address response types --

#[derive(Serialize)]
pub struct AddrTxsResponse {
    /// The queried address
    #[serde(rename = "addrStr")]
    pub addr_str: String,
    /// Total number of transactions involving this address
    #[serde(rename = "txCount")]
    pub tx_count: usize,
    /// Transaction IDs (display order hex)
    pub txids: Vec<String>,
}

#[derive(Serialize)]
pub struct AddrSummaryResponse {
    /// The queried address
    #[serde(rename = "addrStr")]
    pub addr_str: String,
    /// Total number of transactions involving this address
    #[serde(rename = "txCount")]
    pub tx_count: usize,
}

/// GET /api/addr/:addr
///
/// Returns a summary for the given Dash address.
pub async fn get_addr_summary(
    State(state): State<Arc<AppState>>,
    Path(addr_str): Path<String>,
) -> Result<Json<AddrSummaryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let (_version, addr_hash) = decode_address(&addr_str).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid Dash address: {}", addr_str),
            }),
        )
    })?;

    let tx_refs = state.db.get_addr_txs(&addr_hash).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Database error".to_string(),
            }),
        )
    })?;

    Ok(Json(AddrSummaryResponse {
        addr_str,
        tx_count: tx_refs.len(),
    }))
}

/// GET /api/addr/:addr/txs
///
/// Returns all transaction IDs for the given Dash address, ordered by block height.
pub async fn get_addr_txs(
    State(state): State<Arc<AppState>>,
    Path(addr_str): Path<String>,
) -> Result<Json<AddrTxsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let (_version, addr_hash) = decode_address(&addr_str).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid Dash address: {}", addr_str),
            }),
        )
    })?;

    let tx_refs = state.db.get_addr_txs(&addr_hash).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Database error".to_string(),
            }),
        )
    })?;

    // Deduplicate txids (same tx can appear multiple times if it has
    // multiple outputs to the same address)
    let mut seen = std::collections::HashSet::new();
    let txids: Vec<String> = tx_refs
        .iter()
        .filter(|r| seen.insert(r.txid))
        .map(|r| hash_to_display(&r.txid))
        .collect();

    Ok(Json(AddrTxsResponse {
        addr_str,
        tx_count: txids.len(),
        txids,
    }))
}

fn block_to_response(block: &daino_state::db::BlockRecord) -> BlockResponse {
    BlockResponse {
        hash: hash_to_display(&block.hash),
        height: block.height,
        previous_block_hash: hash_to_display(&block.prev_hash),
        merkle_root: hash_to_display(&block.merkle_root),
        time: block.time,
        bits: format!("{:08x}", block.bits),
        nonce: block.nonce,
        tx_count: block.tx_count,
        size: block.size,
    }
}
