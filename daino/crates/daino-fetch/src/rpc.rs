//! Dash Core JSON-RPC client.
//!
//! Provides a typed interface to the dashd JSON-RPC API.
//! Used for:
//! - Querying current chain tip (for tip-following)
//! - Fetching individual blocks/transactions
//! - Checking ChainLock and InstantSend status
//! - Reading governance and spork data

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

/// A JSON-RPC client connected to a dashd instance.
pub struct DashdRpc {
    /// Base URL, e.g. "http://127.0.0.1:9998"
    url: String,
    /// HTTP client
    client: reqwest::Client,
    /// RPC authentication (user:pass)
    auth: Option<(String, String)>,
    /// Request ID counter
    id: std::sync::atomic::AtomicU64,
}

/// Blockchain info from `getblockchaininfo`.
#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainInfo {
    pub chain: String,
    pub blocks: u64,
    pub headers: u64,
    #[serde(rename = "bestblockhash")]
    pub best_block_hash: String,
    pub difficulty: f64,
    #[serde(rename = "verificationprogress")]
    pub verification_progress: f64,
    #[serde(rename = "initialblockdownload")]
    pub initial_block_download: bool,
}

/// Block header info from `getblock` with verbosity=1.
#[derive(Debug, Clone, Deserialize)]
pub struct RpcBlock {
    pub hash: String,
    #[serde(rename = "confirmations")]
    pub confirmations: i64,
    pub height: u64,
    pub version: u32,
    #[serde(rename = "merkleroot")]
    pub merkle_root: String,
    pub time: u64,
    pub nonce: u64,
    pub bits: String,
    pub difficulty: f64,
    #[serde(rename = "previousblockhash")]
    pub previous_block_hash: Option<String>,
    #[serde(rename = "nextblockhash")]
    pub next_block_hash: Option<String>,
    /// Transaction IDs in this block (with verbosity=1)
    pub tx: Vec<String>,
    pub size: u64,
    /// ChainLock status
    #[serde(rename = "chainlock")]
    pub chainlock: Option<bool>,
}

/// Best ChainLock info from `getbestchainlock`.
#[derive(Debug, Clone, Deserialize)]
pub struct BestChainLock {
    #[serde(rename = "blockhash")]
    pub block_hash: String,
    pub height: u64,
    pub signature: String,
    #[serde(rename = "known_block")]
    pub known_block: bool,
}

/// Raw transaction info from `getrawtransaction` with verbose=true.
#[derive(Debug, Clone, Deserialize)]
pub struct RpcTransaction {
    pub txid: String,
    pub version: i32,
    #[serde(rename = "type")]
    pub tx_type: u32,
    pub size: u64,
    pub locktime: u64,
    pub vin: Vec<RpcVin>,
    pub vout: Vec<RpcVout>,
    #[serde(rename = "blockhash")]
    pub block_hash: Option<String>,
    pub confirmations: Option<i64>,
    pub time: Option<u64>,
    /// InstantSend lock status
    #[serde(rename = "instantlock")]
    pub instant_lock: Option<bool>,
    #[serde(rename = "instantlock_internal")]
    pub instant_lock_internal: Option<bool>,
    #[serde(rename = "chainlock")]
    pub chainlock: Option<bool>,
}

/// Transaction input from RPC.
#[derive(Debug, Clone, Deserialize)]
pub struct RpcVin {
    pub txid: Option<String>,
    pub vout: Option<u32>,
    pub coinbase: Option<String>,
}

/// Transaction output from RPC.
#[derive(Debug, Clone, Deserialize)]
pub struct RpcVout {
    pub value: f64,
    pub n: u32,
    #[serde(rename = "scriptPubKey")]
    pub script_pub_key: RpcScriptPubKey,
}

/// Script details from RPC.
#[derive(Debug, Clone, Deserialize)]
pub struct RpcScriptPubKey {
    pub hex: String,
    #[serde(rename = "type")]
    pub script_type: Option<String>,
    pub address: Option<String>,
    pub addresses: Option<Vec<String>>,
}

/// Spork data from `spork show`.
#[derive(Debug, Clone, Deserialize)]
pub struct SporkInfo(pub std::collections::HashMap<String, Value>);

impl DashdRpc {
    /// Create a new RPC client.
    ///
    /// # Arguments
    /// * `url` - Base URL (e.g. "http://127.0.0.1:9998")
    /// * `user` - RPC username (from dash.conf `rpcuser`)
    /// * `password` - RPC password (from dash.conf `rpcpassword`)
    pub fn new(url: &str, user: &str, password: &str) -> Self {
        DashdRpc {
            url: url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            auth: Some((user.to_string(), password.to_string())),
            id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Create a client without authentication (for cookie-based auth).
    pub fn new_no_auth(url: &str) -> Self {
        DashdRpc {
            url: url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            auth: None,
            id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Send a raw JSON-RPC request and return the result field.
    async fn call(&self, method: &str, params: &[Value]) -> Result<Value> {
        let id = self
            .id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let body = json!({
            "jsonrpc": "1.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let mut req = self.client.post(&self.url);
        if let Some((user, pass)) = &self.auth {
            req = req.basic_auth(user, Some(pass));
        }

        let resp = req
            .json(&body)
            .send()
            .await
            .with_context(|| format!("RPC call to {} failed", method))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .with_context(|| format!("Failed to read RPC response for {}", method))?;

        let parsed: Value = serde_json::from_str(&text)
            .with_context(|| format!("Invalid JSON from {}: {}", method, &text[..text.len().min(200)]))?;

        // Check for JSON-RPC error
        if let Some(err) = parsed.get("error") {
            if !err.is_null() {
                let msg = err
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown RPC error");
                let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
                anyhow::bail!("RPC error {} (code {}): {}", method, code, msg);
            }
        }

        if !status.is_success() {
            anyhow::bail!("RPC {} returned HTTP {}: {}", method, status, &text[..text.len().min(200)]);
        }

        parsed
            .get("result")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No 'result' in RPC response for {}", method))
    }

    /// Get blockchain info (current tip, sync status, etc.).
    pub async fn get_blockchain_info(&self) -> Result<BlockchainInfo> {
        let result = self.call("getblockchaininfo", &[]).await?;
        Ok(serde_json::from_value(result)?)
    }

    /// Get block hash at the given height.
    pub async fn get_block_hash(&self, height: u64) -> Result<String> {
        let result = self.call("getblockhash", &[json!(height)]).await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Expected string from getblockhash"))
    }

    /// Get block data by hash (verbosity=1: JSON with tx IDs).
    pub async fn get_block(&self, hash: &str) -> Result<RpcBlock> {
        let result = self.call("getblock", &[json!(hash), json!(1)]).await?;
        Ok(serde_json::from_value(result)?)
    }

    /// Get raw block hex by hash (verbosity=0).
    pub async fn get_block_hex(&self, hash: &str) -> Result<String> {
        let result = self.call("getblock", &[json!(hash), json!(0)]).await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Expected string from getblock"))
    }

    /// Get transaction by txid (verbose=true: JSON).
    pub async fn get_raw_transaction(&self, txid: &str) -> Result<RpcTransaction> {
        let result = self
            .call("getrawtransaction", &[json!(txid), json!(true)])
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    /// Get raw transaction hex by txid (verbose=false).
    pub async fn get_raw_transaction_hex(&self, txid: &str) -> Result<String> {
        let result = self
            .call("getrawtransaction", &[json!(txid), json!(false)])
            .await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Expected string from getrawtransaction"))
    }

    /// Get the best ChainLock info.
    pub async fn get_best_chainlock(&self) -> Result<BestChainLock> {
        let result = self.call("getbestchainlock", &[]).await?;
        Ok(serde_json::from_value(result)?)
    }

    /// Get the current block count (tip height).
    pub async fn get_block_count(&self) -> Result<u64> {
        let result = self.call("getblockcount", &[]).await?;
        result
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Expected number from getblockcount"))
    }

    /// Check if dashd is reachable and responding.
    pub async fn ping(&self) -> Result<()> {
        self.call("getblockchaininfo", &[]).await?;
        Ok(())
    }

    /// Make a raw RPC call and return the result as untyped JSON.
    ///
    /// Useful for Dash-specific RPCs (spork, gobject, etc.) where we
    /// don't need to deserialize into a specific struct.
    pub async fn call_raw(&self, method: &str, params: &[Value]) -> Result<Value> {
        self.call(method, params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_client_creation() {
        let rpc = DashdRpc::new("http://127.0.0.1:9998", "user", "pass");
        assert_eq!(rpc.url, "http://127.0.0.1:9998");
        assert!(rpc.auth.is_some());
    }

    #[test]
    fn test_rpc_client_no_auth() {
        let rpc = DashdRpc::new_no_auth("http://127.0.0.1:9998/");
        assert_eq!(rpc.url, "http://127.0.0.1:9998");
        assert!(rpc.auth.is_none());
    }
}
