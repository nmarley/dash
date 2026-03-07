//! REST API endpoint handlers.
//!
//! Modeled after the Insight API:
//! - GET /api/status            -- chain status (indexed + optional dashd live)
//! - GET /api/block/:hash       -- block by hash
//! - GET /api/block-index/:h    -- block hash by height
//! - GET /api/tx/:txid          -- transaction by txid
//! - GET /api/addr/:addr        -- address summary
//! - GET /api/addr/:addr/txs    -- address transaction history
//! - GET /api/chainlock         -- best ChainLock (dashd live)
//! - GET /api/governance/list   -- governance proposals (dashd live)
//! - GET /api/sporks            -- active sporks (dashd live)

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::Serialize;

use daino_fetch::rpc::DashdRpc;
use daino_state::db::DainoDB;
use librustdash::Transaction;
use librustdash::hash::{hash_to_display, reverse_hash};
use librustdash::script::{ScriptType, analyze_script, decode_address, encode_address};

/// Shared application state passed to all handlers.
pub struct AppState {
    pub db: DainoDB,
    /// Optional connection to a running dashd for live queries.
    /// When None, dashd-backed endpoints return 503 Service Unavailable.
    pub rpc: Option<DashdRpc>,
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
    pub size: u32,
    pub height: u32,
    pub version: i32,
    #[serde(rename = "merkleroot")]
    pub merkle_root: String,
    /// Transaction IDs in this block (display-order hex)
    pub tx: Vec<String>,
    pub time: u32,
    pub nonce: u32,
    pub bits: String,
    pub difficulty: f64,
    /// Block reward in DASH (coinbase output value, 8 decimal places)
    pub reward: String,
    /// Cumulative proof-of-work (hex, no leading zeros)
    pub chainwork: String,
    pub confirmations: u32,
    #[serde(rename = "previousblockhash")]
    pub previous_block_hash: String,
    /// Next block hash (omitted if this is the tip)
    #[serde(rename = "nextblockhash", skip_serializing_if = "Option::is_none")]
    pub next_block_hash: Option<String>,
    /// ChainLock status (None if dashd not connected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chainlock: Option<bool>,
}

#[derive(Serialize)]
pub struct BlockHashResponse {
    #[serde(rename = "blockHash")]
    pub block_hash: String,
}

#[derive(Serialize)]
pub struct TxResponse {
    pub txid: String,
    pub version: i16,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tx_type: Option<u16>,
    pub locktime: u32,
    pub vin: Vec<TxVin>,
    pub vout: Vec<TxVout>,
    pub blockhash: String,
    #[serde(rename = "blockheight")]
    pub block_height: i32,
    pub confirmations: u32,
    pub time: u32,
    pub blocktime: u32,
    #[serde(rename = "isCoinBase", skip_serializing_if = "Option::is_none")]
    pub is_coinbase: Option<bool>,
    #[serde(rename = "valueOut")]
    pub value_out: f64,
    pub size: usize,
    #[serde(rename = "valueIn", skip_serializing_if = "Option::is_none")]
    pub value_in: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fees: Option<f64>,
    #[serde(rename = "txlock", skip_serializing_if = "Option::is_none")]
    pub txlock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chainlock: Option<bool>,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum TxVin {
    Coinbase {
        coinbase: String,
        sequence: u32,
        n: u32,
    },
    Regular {
        txid: String,
        vout: u32,
        sequence: u32,
        n: u32,
        #[serde(rename = "scriptSig")]
        script_sig: ScriptSigResponse,
        #[serde(skip_serializing_if = "Option::is_none")]
        addr: Option<String>,
        #[serde(rename = "valueSat", skip_serializing_if = "Option::is_none")]
        value_sat: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<f64>,
        #[serde(rename = "doubleSpentTxID")]
        double_spent_tx_id: Option<()>,
    },
}

#[derive(Serialize)]
pub struct ScriptSigResponse {
    pub hex: String,
}

#[derive(Serialize)]
pub struct TxVout {
    pub value: String,
    pub n: u32,
    #[serde(rename = "scriptPubKey")]
    pub script_pub_key: ScriptPubKeyResponse,
    #[serde(rename = "spentTxId")]
    pub spent_tx_id: Option<String>,
    #[serde(rename = "spentIndex")]
    pub spent_index: Option<u32>,
    #[serde(rename = "spentHeight")]
    pub spent_height: Option<u32>,
}

#[derive(Serialize)]
pub struct ScriptPubKeyResponse {
    pub hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addresses: Option<Vec<String>>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub script_type: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

fn db_error() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "Database error".to_string(),
        }),
    )
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

    // Fetch txid list for this block
    let raw_txids = state
        .db
        .get_block_txids(block.height)
        .map_err(|_| db_error())?;

    let txids: Vec<String> = raw_txids.iter().map(hash_to_display).collect();

    // Look up coinbase (first tx) to get block reward
    let reward = if let Some(coinbase_txid) = raw_txids.first() {
        state
            .db
            .get_tx(coinbase_txid)
            .ok()
            .flatten()
            .map(|tx| format!("{:.8}", tx.value_out as f64 / 100_000_000.0))
            .unwrap_or_else(|| "0.00000000".to_string())
    } else {
        "0.00000000".to_string()
    };

    // Fetch next block hash (height + 1)
    let next_block_hash = state
        .db
        .get_block_by_height(block.height + 1)
        .ok()
        .flatten()
        .map(|b| hash_to_display(&b.hash));

    // Compute confirmations from tip
    let tip_height = state
        .db
        .get_meta()
        .ok()
        .flatten()
        .map(|m| m.tip_height)
        .unwrap_or(block.height);
    let confirmations = tip_height - block.height + 1;

    // Query dashd for ChainLock status if available
    let chainlock = query_block_chainlock(&state.rpc, &hash_hex).await;

    Ok(Json(block_to_response(
        &block,
        txids,
        next_block_hash,
        confirmations,
        chainlock,
        reward,
    )))
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

    let tx_record = state
        .db
        .get_tx(&internal_txid)
        .map_err(|_| db_error())?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Transaction not found: {}", txid_hex),
                }),
            )
        })?;

    // Get raw transaction bytes and deserialize
    let raw_bytes = state
        .db
        .get_raw_tx(&internal_txid)
        .map_err(|_| db_error())?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Raw tx data not found: {}", txid_hex),
                }),
            )
        })?;

    let tx_size = raw_bytes.len();

    let tx = Transaction::deserialize(&raw_bytes).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to deserialize transaction".to_string(),
            }),
        )
    })?;

    // Get block info for blockhash and blocktime
    let block_record = state
        .db
        .get_block_by_height(tx_record.block_height)
        .map_err(|_| db_error())?;

    let blockhash = block_record
        .as_ref()
        .map(|b| hash_to_display(&b.hash))
        .unwrap_or_default();

    let blocktime = block_record.as_ref().map(|b| b.time).unwrap_or(0);

    // Compute confirmations
    let tip_height = state
        .db
        .get_meta()
        .ok()
        .flatten()
        .map(|m| m.tip_height)
        .unwrap_or(tx_record.block_height);
    let confirmations = tip_height - tx_record.block_height + 1;

    let is_coinbase = tx.is_coinbase();

    // Build vin array
    let mut vin = Vec::with_capacity(tx.inputs.len());
    let mut value_in_total: i64 = 0;

    for (i, input) in tx.inputs.iter().enumerate() {
        if is_coinbase {
            vin.push(TxVin::Coinbase {
                coinbase: hex::encode(&input.script_sig),
                sequence: input.sequence,
                n: i as u32,
            });
        } else {
            // Look up the spent output's value and address via the UTXO
            // or spent_by DB. We use get_raw_tx on the previous tx to
            // get the output's scriptPubKey and value.
            let prev_txid = input.previous_output.hash;
            let prev_vout = input.previous_output.n;

            let (addr, value_sat) = resolve_input_details(&state.db, &prev_txid, prev_vout);

            if let Some(v) = value_sat {
                value_in_total += v;
            }

            vin.push(TxVin::Regular {
                txid: hash_to_display(&prev_txid),
                vout: prev_vout,
                sequence: input.sequence,
                n: i as u32,
                script_sig: ScriptSigResponse {
                    hex: hex::encode(&input.script_sig),
                },
                addr,
                value_sat,
                value: value_sat.map(|v| v as f64 / 100_000_000.0),
                double_spent_tx_id: None,
            });
        }
    }

    // Build vout array
    let mut vout = Vec::with_capacity(tx.outputs.len());

    for (i, output) in tx.outputs.iter().enumerate() {
        let info = analyze_script(&output.script_pubkey);

        let (addresses, script_type) = match (&info.script_type, &info.address_hash) {
            (ScriptType::P2pkh, Some(hash)) => {
                let addr = encode_address(hash, &info.script_type, false);
                (addr.map(|a| vec![a]), Some("pubkeyhash".to_string()))
            }
            (ScriptType::P2sh, Some(hash)) => {
                let addr = encode_address(hash, &info.script_type, false);
                (addr.map(|a| vec![a]), Some("scripthash".to_string()))
            }
            (ScriptType::P2pk, Some(hash)) => {
                let addr = encode_address(hash, &info.script_type, false);
                (addr.map(|a| vec![a]), Some("pubkey".to_string()))
            }
            (ScriptType::OpReturn, _) => (None, Some("nulldata".to_string())),
            _ => (None, None),
        };

        // Look up spent-by info
        let (spent_tx_id, spent_index, spent_height) =
            match state.db.get_spent_by(&internal_txid, i as u32) {
                Ok(Some((stxid, sidx, sheight))) => {
                    (Some(hash_to_display(&stxid)), Some(sidx), Some(sheight))
                }
                _ => (None, None, None),
            };

        vout.push(TxVout {
            value: format!("{:.8}", output.value as f64 / 100_000_000.0),
            n: i as u32,
            script_pub_key: ScriptPubKeyResponse {
                hex: hex::encode(&output.script_pubkey),
                addresses,
                script_type,
            },
            spent_tx_id,
            spent_index,
            spent_height,
        });
    }

    let value_out = tx_record.value_out as f64 / 100_000_000.0;

    let (value_in, fees) = if is_coinbase {
        (None, None)
    } else {
        let vi = value_in_total as f64 / 100_000_000.0;
        (Some(vi), Some(vi - value_out))
    };

    // Query dashd for InstantSend/ChainLock status if available
    let (txlock, chainlock) = query_tx_locks(&state.rpc, &txid_hex).await;

    Ok(Json(TxResponse {
        txid: hash_to_display(&tx_record.txid),
        version: tx_record.version,
        tx_type: if tx_record.tx_type > 0 {
            Some(tx_record.tx_type)
        } else {
            None
        },
        locktime: tx_record.lock_time,
        vin,
        vout,
        blockhash,
        block_height: tx_record.block_height as i32,
        confirmations,
        time: blocktime,
        blocktime,
        is_coinbase: if is_coinbase { Some(true) } else { None },
        value_out,
        size: tx_size,
        value_in,
        fees,
        txlock,
        chainlock,
    }))
}

/// Resolve input address and value by looking up the previous transaction's output.
fn resolve_input_details(
    db: &DainoDB,
    prev_txid: &[u8; 32],
    prev_vout: u32,
) -> (Option<String>, Option<i64>) {
    // First try: look up the previous tx's raw bytes and extract the output
    if let Ok(Some(raw)) = db.get_raw_tx(prev_txid)
        && let Ok(prev_tx) = Transaction::deserialize(&raw)
        && let Some(output) = prev_tx.outputs.get(prev_vout as usize)
    {
        let info = analyze_script(&output.script_pubkey);
        let addr = info
            .address_hash
            .and_then(|h| encode_address(&h, &info.script_type, false));
        return (addr, Some(output.value));
    }

    // Fallback: check if there's still a live UTXO (shouldn't be, since it's spent)
    if let Ok(Some(utxo)) = db.get_utxo(prev_txid, prev_vout) {
        let addr = utxo
            .addr_hash
            .and_then(|h| encode_address(&h, &ScriptType::P2pkh, false));
        return (addr, Some(utxo.value));
    }

    (None, None)
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

/// GET /api/addr/:addr/utxo
///
/// Returns all unspent transaction outputs for the given Dash address.
pub async fn get_addr_utxos(
    State(state): State<Arc<AppState>>,
    Path(addr_str): Path<String>,
) -> Result<Json<Vec<UtxoResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let (_version, addr_hash) = decode_address(&addr_str).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid Dash address: {}", addr_str),
            }),
        )
    })?;

    let utxos = state.db.get_addr_utxos(&addr_hash).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Database error".to_string(),
            }),
        )
    })?;

    let result: Vec<UtxoResponse> = utxos
        .iter()
        .map(|u| UtxoResponse {
            txid: hash_to_display(&u.txid),
            vout: u.vout,
            value: u.value,
            satoshis: u.value,
            height: u.block_height,
        })
        .collect();

    Ok(Json(result))
}

#[derive(Serialize)]
pub struct UtxoResponse {
    pub txid: String,
    pub vout: u32,
    /// Value in satoshis
    pub value: i64,
    /// Value in satoshis (Insight API compatibility)
    pub satoshis: i64,
    /// Block height where this UTXO was created
    pub height: u32,
}

// -- Dashd-backed response types --

#[derive(Serialize)]
pub struct ChainLockResponse {
    #[serde(rename = "blockhash")]
    pub block_hash: String,
    pub height: u64,
    pub signature: String,
}

#[derive(Serialize)]
pub struct SporkResponse {
    pub sporks: serde_json::Value,
}

#[derive(Serialize)]
pub struct GovernanceListResponse {
    pub proposals: serde_json::Value,
}

// -- Dashd-backed handlers --

/// GET /api/chainlock
///
/// Returns the current best ChainLock from dashd.
pub async fn get_chainlock(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ChainLockResponse>, (StatusCode, Json<ErrorResponse>)> {
    let rpc = state.rpc.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "dashd not connected".to_string(),
            }),
        )
    })?;

    let cl = rpc.get_best_chainlock().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("dashd error: {}", e),
            }),
        )
    })?;

    Ok(Json(ChainLockResponse {
        block_hash: cl.block_hash,
        height: cl.height,
        signature: cl.signature,
    }))
}

/// GET /api/sporks
///
/// Returns active sporks from dashd.
pub async fn get_sporks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SporkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let rpc = state.rpc.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "dashd not connected".to_string(),
            }),
        )
    })?;

    let sporks = rpc
        .call_raw("spork", &[serde_json::json!("show")])
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: format!("dashd error: {}", e),
                }),
            )
        })?;

    Ok(Json(SporkResponse { sporks }))
}

/// GET /api/governance/list
///
/// Returns governance proposals from dashd.
pub async fn get_governance_list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<GovernanceListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let rpc = state.rpc.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "dashd not connected".to_string(),
            }),
        )
    })?;

    let proposals = rpc
        .call_raw(
            "gobject",
            &[
                serde_json::json!("list"),
                serde_json::json!("valid"),
                serde_json::json!("proposals"),
            ],
        )
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: format!("dashd error: {}", e),
                }),
            )
        })?;

    Ok(Json(GovernanceListResponse { proposals }))
}

// -- Helper functions --

/// Query dashd for a block's ChainLock status. Returns None if dashd not connected or error.
async fn query_block_chainlock(rpc: &Option<DashdRpc>, hash_hex: &str) -> Option<bool> {
    let rpc = rpc.as_ref()?;
    let block = rpc.get_block(hash_hex).await.ok()?;
    block.chainlock
}

/// Query dashd for a transaction's InstantSend and ChainLock status.
async fn query_tx_locks(rpc: &Option<DashdRpc>, txid_hex: &str) -> (Option<bool>, Option<bool>) {
    let rpc = match rpc.as_ref() {
        Some(r) => r,
        None => return (None, None),
    };
    match rpc.get_raw_transaction(txid_hex).await {
        Ok(tx) => (tx.instant_lock, tx.chainlock),
        Err(_) => (None, None),
    }
}

fn block_to_response(
    block: &daino_state::db::BlockRecord,
    txids: Vec<String>,
    next_block_hash: Option<String>,
    confirmations: u32,
    chainlock: Option<bool>,
    reward: String,
) -> BlockResponse {
    use daino_core::{difficulty_from_bits, u256_to_hex};

    BlockResponse {
        hash: hash_to_display(&block.hash),
        size: block.size,
        height: block.height,
        version: block.version,
        merkle_root: hash_to_display(&block.merkle_root),
        tx: txids,
        time: block.time,
        nonce: block.nonce,
        bits: format!("{:08x}", block.bits),
        difficulty: difficulty_from_bits(block.bits),
        reward,
        chainwork: u256_to_hex(&block.chainwork),
        confirmations,
        previous_block_hash: hash_to_display(&block.prev_hash),
        next_block_hash,
        chainlock,
    }
}
