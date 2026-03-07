//! Integration test: index blocks from test data, then query the REST API.
//!
//! This test reads real blocks from data/blk00000.dat, indexes them into
//! a temporary LMDB database, starts the API server, and verifies responses.

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use tower_http::cors::CorsLayer;

use daino_core::{BlockFileReader, Network, add_u256, work_from_bits};
use daino_state::db::{
    AddrTxRef, BlockBatch, BlockRecord, DainoDB, SpentByEntry, SpentOutpoint, TxRecord, UtxoEntry,
};
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
    let mut prev_chainwork = [0u8; 32];

    for height in 0..n {
        let block = reader
            .read_next_block()
            .unwrap()
            .expect("Expected more blocks in test data");

        let block_hash = block.header.block_hash().unwrap();
        let block_size = block.serialize().unwrap().len() as u32;

        let chainwork = add_u256(&prev_chainwork, &work_from_bits(block.header.bits));
        prev_chainwork = chainwork;

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
            chainwork,
        };

        let mut tx_records = Vec::new();
        let mut addr_refs: Vec<([u8; 20], AddrTxRef)> = Vec::new();
        let mut new_utxos: Vec<UtxoEntry> = Vec::new();
        let mut spent: Vec<SpentOutpoint> = Vec::new();
        let mut raw_txs: Vec<([u8; 32], Vec<u8>)> = Vec::new();
        let mut spent_by: Vec<SpentByEntry> = Vec::new();

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

            // Store raw serialized transaction bytes
            raw_txs.push((txid, tx.serialize().unwrap()));

            if !tx.is_coinbase() {
                for (vin_idx, input) in tx.inputs.iter().enumerate() {
                    spent.push(SpentOutpoint {
                        txid: input.previous_output.hash,
                        vout: input.previous_output.n,
                    });

                    // Track which tx spent each output
                    spent_by.push(SpentByEntry {
                        spent_txid: input.previous_output.hash,
                        spent_vout: input.previous_output.n,
                        spending_txid: txid,
                        spending_vin: vin_idx as u32,
                        spending_height: height as u32,
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

        db.put_batch(&[BlockBatch {
            block: block_record,
            txs: tx_records,
            addr_refs,
            new_utxos,
            spent,
            raw_txs,
            spent_by,
        }])
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

    // Verify new fields: difficulty, reward, chainwork
    let diff = body["difficulty"]
        .as_f64()
        .expect("difficulty should be f64");
    let expected_diff = 0.000244140625; // 1/4096, Dash genesis relative to Bitcoin difficulty-1
    assert!(
        (diff - expected_diff).abs() < 1e-12,
        "genesis difficulty should be {}, got {}",
        expected_diff,
        diff
    );

    let reward = body["reward"].as_str().expect("reward should be string");
    assert_eq!(
        reward, "50.00000000",
        "genesis reward should be 50.00000000, got {}",
        reward
    );

    let chainwork = body["chainwork"]
        .as_str()
        .expect("chainwork should be string");
    assert_eq!(
        chainwork.len(),
        64,
        "chainwork should be 64 hex chars, got {}",
        chainwork.len()
    );
    assert_ne!(
        chainwork, "0000000000000000000000000000000000000000000000000000000000000000",
        "chainwork should not be zero for genesis"
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
async fn test_query_tx_coinbase_response() {
    let dir = tempfile::tempdir().unwrap();
    let db = DainoDB::open(dir.path()).unwrap();

    let block_hashes = index_test_blocks(&db, 5);
    let genesis_hash_hex = hash_to_display(&block_hashes[0]);

    let base_url = start_test_server(db).await;

    // Get the genesis block to find the coinbase txid
    let resp = reqwest::get(format!("{}/api/block/{}", base_url, genesis_hash_hex))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let block_body: serde_json::Value = resp.json().await.unwrap();
    let coinbase_txid = block_body["tx"][0].as_str().unwrap().to_string();

    // GET /api/tx/:txid for the genesis coinbase
    let resp = reqwest::get(format!("{}/api/tx/{}", base_url, coinbase_txid))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let tx: serde_json::Value = resp.json().await.unwrap();

    // Core fields
    assert_eq!(tx["txid"], coinbase_txid);
    assert_eq!(tx["blockhash"], genesis_hash_hex);
    assert_eq!(tx["blockheight"], 0);
    assert!(tx["confirmations"].as_u64().unwrap() >= 5);
    assert!(tx["time"].as_u64().unwrap() > 0);
    assert!(tx["blocktime"].as_u64().unwrap() > 0);
    assert_eq!(tx["time"], tx["blocktime"]);

    // Coinbase flag
    assert_eq!(tx["isCoinBase"], true);

    // Coinbase vin: should have exactly one input with "coinbase" field
    let vin = tx["vin"].as_array().unwrap();
    assert_eq!(vin.len(), 1);
    assert!(
        vin[0].get("coinbase").is_some(),
        "coinbase input should have 'coinbase' field"
    );
    assert!(
        vin[0].get("sequence").is_some(),
        "coinbase input should have 'sequence' field"
    );
    assert_eq!(vin[0]["n"], 0);
    // Coinbase inputs should NOT have txid/vout/scriptSig
    assert!(vin[0].get("txid").is_none());
    assert!(vin[0].get("vout").is_none());
    assert!(vin[0].get("scriptSig").is_none());

    // Vout: should have at least one output
    let vout = tx["vout"].as_array().unwrap();
    assert!(!vout.is_empty());
    assert_eq!(vout[0]["n"], 0);

    // vout[0].value should be a string with 8 decimal places
    let val_str = vout[0]["value"].as_str().unwrap();
    assert!(
        val_str.contains('.'),
        "value should be a decimal string, got: {}",
        val_str
    );
    let parts: Vec<&str> = val_str.split('.').collect();
    assert_eq!(
        parts[1].len(),
        8,
        "value should have 8 decimal places, got: {}",
        val_str
    );

    // scriptPubKey should have hex and type
    assert!(!vout[0]["scriptPubKey"]["hex"].as_str().unwrap().is_empty());
    assert!(vout[0]["scriptPubKey"]["type"].as_str().is_some());

    // Coinbase tx should not have valueIn or fees
    assert!(tx.get("valueIn").is_none() || tx["valueIn"].is_null());
    assert!(tx.get("fees").is_none() || tx["fees"].is_null());

    // valueOut should be a positive number
    let value_out = tx["valueOut"].as_f64().unwrap();
    assert!(value_out > 0.0, "valueOut should be positive");

    // size should be positive
    let size = tx["size"].as_u64().unwrap();
    assert!(size > 0, "tx size should be positive");
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
