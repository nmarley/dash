//! Integration test: index blocks from test data, then query the REST API.
//!
//! This test reads real blocks from data/blk00000.dat, indexes them into
//! a temporary LMDB database, starts the API server, and verifies responses.

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use tower_http::cors::CorsLayer;

use daino_core::{BlockFileReader, Network};
use daino_state::db::{AddrTxRef, BlockRecord, DainoDB, SpentOutpoint, TxRecord, UtxoEntry};
use librustdash::hash::hash_to_display;
use librustdash::script::analyze_script;

/// Index N blocks from the test data into a temp database.
fn index_test_blocks(db: &DainoDB, n: usize) -> Vec<[u8; 32]> {
    let block_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/blk00000.dat");

    if !block_file.exists() {
        panic!(
            "Test data not found at {:?}. Copy blk00000.dat to data/",
            block_file
        );
    }

    let mut reader = BlockFileReader::new(&block_file, Network::Mainnet).unwrap();
    let mut block_hashes = Vec::new();

    for height in 0..n {
        let block = reader
            .read_next_block()
            .unwrap()
            .expect("Expected more blocks in test data");

        let block_hash = block.header.block_hash().unwrap();
        let block_size = block.serialize().unwrap().len() as u32;

        let block_record = BlockRecord {
            height: height as u32,
            hash: block_hash,
            prev_hash: block.header.prev_blockhash,
            merkle_root: block.header.merkle_root,
            version: block.header.version,
            time: block.header.time,
            bits: block.header.bits,
            nonce: block.header.nonce,
            tx_count: block.transactions.len() as u32,
            size: block_size,
        };

        let mut tx_records = Vec::new();
        let mut addr_refs: Vec<([u8; 20], AddrTxRef)> = Vec::new();
        let mut new_utxos: Vec<UtxoEntry> = Vec::new();
        let mut spent: Vec<SpentOutpoint> = Vec::new();

        for (tx_idx, tx) in block.transactions.iter().enumerate() {
            let txid = tx.txid().unwrap();
            let value_out: i64 = tx.outputs.iter().map(|o| o.value).sum();

            tx_records.push(TxRecord {
                txid,
                block_height: height as u32,
                tx_index: tx_idx as u32,
                version: tx.version,
                tx_type: tx.tx_type as u16,
                lock_time: tx.lock_time,
                value_out,
                input_count: tx.inputs.len() as u32,
                output_count: tx.outputs.len() as u32,
            });

            if !tx.is_coinbase() {
                for input in &tx.inputs {
                    spent.push(SpentOutpoint {
                        txid: input.previous_output.hash,
                        vout: input.previous_output.n,
                    });
                }
            }

            for (vout, output) in tx.outputs.iter().enumerate() {
                let info = analyze_script(&output.script_pubkey);
                let addr_hash = info.address_hash;

                if let Some(ah) = addr_hash {
                    addr_refs.push((
                        ah,
                        AddrTxRef {
                            block_height: height as u32,
                            txid,
                        },
                    ));
                }

                new_utxos.push(UtxoEntry {
                    txid,
                    vout: vout as u32,
                    value: output.value,
                    block_height: height as u32,
                    addr_hash,
                });
            }
        }

        db.put_block(&block_record, &tx_records, &addr_refs, &new_utxos, &spent)
            .unwrap();
        block_hashes.push(block_hash);
    }

    block_hashes
}

/// Start the API server on a random port and return the base URL.
async fn start_test_server(db: DainoDB) -> String {
    use daino_serve::api::{self, AppState};

    let state = Arc::new(AppState { db, rpc: None });

    let app = Router::new()
        .route("/api/status", get(api::get_status))
        .route("/api/block/{hash}", get(api::get_block_by_hash))
        .route("/api/block-index/{height}", get(api::get_block_by_height))
        .route("/api/tx/{txid}", get(api::get_tx))
        .route("/api/addr/{addr}", get(api::get_addr_summary))
        .route("/api/addr/{addr}/txs", get(api::get_addr_txs))
        .route("/api/addr/{addr}/utxo", get(api::get_addr_utxos))
        .route("/api/chainlock", get(api::get_chainlock))
        .route("/api/sporks", get(api::get_sporks))
        .route("/api/governance/list", get(api::get_governance_list))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    base_url
}

#[tokio::test]
async fn test_index_and_query_status() {
    let dir = tempfile::tempdir().unwrap();
    let db = DainoDB::open(dir.path()).unwrap();

    let block_hashes = index_test_blocks(&db, 10);
    assert_eq!(block_hashes.len(), 10);

    let base_url = start_test_server(db).await;

    // GET /api/status
    let resp = reqwest::get(format!("{}/api/status", base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["info"]["blocks"], 9); // tip_height = last block = 9
    assert_eq!(body["info"]["txcount"], 10); // one coinbase tx per block
}

#[tokio::test]
async fn test_query_genesis_block() {
    let dir = tempfile::tempdir().unwrap();
    let db = DainoDB::open(dir.path()).unwrap();

    let block_hashes = index_test_blocks(&db, 1);
    let genesis_hash_hex = hash_to_display(&block_hashes[0]);

    let base_url = start_test_server(db).await;

    // GET /api/block/:hash -- query genesis block
    let resp = reqwest::get(format!("{}/api/block/{}", base_url, genesis_hash_hex))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["height"], 0);
    assert_eq!(body["hash"], genesis_hash_hex);

    // Verify this is the actual Dash genesis block hash
    assert_eq!(
        genesis_hash_hex,
        "00000ffd590b1485b3caadc19b22e6379c733355108f107a430458cdf3407ab6"
    );
}

#[tokio::test]
async fn test_query_block_by_height() {
    let dir = tempfile::tempdir().unwrap();
    let db = DainoDB::open(dir.path()).unwrap();

    let block_hashes = index_test_blocks(&db, 5);

    let base_url = start_test_server(db).await;

    // GET /api/block-index/0
    let resp = reqwest::get(format!("{}/api/block-index/0", base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["blockHash"], hash_to_display(&block_hashes[0]));

    // GET /api/block-index/4
    let resp = reqwest::get(format!("{}/api/block-index/4", base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["blockHash"], hash_to_display(&block_hashes[4]));

    // GET /api/block-index/999 -- not found
    let resp = reqwest::get(format!("{}/api/block-index/999", base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_query_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let db = DainoDB::open(dir.path()).unwrap();
    index_test_blocks(&db, 1);

    let base_url = start_test_server(db).await;

    // Non-existent block hash
    let fake_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let resp = reqwest::get(format!("{}/api/block/{}", base_url, fake_hash))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // Non-existent txid
    let resp = reqwest::get(format!("{}/api/tx/{}", base_url, fake_hash))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_dashd_endpoints_unavailable_without_rpc() {
    let dir = tempfile::tempdir().unwrap();
    let db = DainoDB::open(dir.path()).unwrap();
    index_test_blocks(&db, 1);

    let base_url = start_test_server(db).await;

    // Without dashd RPC configured, these should return 503
    let resp = reqwest::get(format!("{}/api/chainlock", base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 503);

    let resp = reqwest::get(format!("{}/api/sporks", base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 503);

    let resp = reqwest::get(format!("{}/api/governance/list", base_url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 503);
}
