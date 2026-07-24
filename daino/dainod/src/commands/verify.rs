//! The `verify` subcommand -- cross-check the local index against dashd RPC.
//!
//! Compares block hashes, tx counts, and sample txids at selected heights.
//! Exits non-zero if any mismatch is found.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use librustdash::hash::hash_to_display;

use daino_fetch::rpc::DashdRpc;
use daino_state::db::DainoDB;

/// One height that failed verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyMismatch {
    pub height: u32,
    pub detail: String,
}

/// Result of verifying a set of heights.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    pub checked: u32,
    pub mismatches: Vec<VerifyMismatch>,
}

impl VerifyReport {
    pub fn ok(&self) -> bool {
        self.mismatches.is_empty()
    }
}

/// Choose heights to verify across `[0, tip]` inclusive.
///
/// Always includes genesis (0) and tip when tip > 0. Fills remaining
/// slots with evenly spaced heights (and optional extras).
pub fn select_heights(tip: u32, count: u32, extra: &[u32]) -> Vec<u32> {
    let mut set = BTreeSet::new();
    set.insert(0);
    if tip > 0 {
        set.insert(tip);
    }
    for &h in extra {
        if h <= tip {
            set.insert(h);
        }
    }

    let target = count.max(2) as usize;
    if tip > 0 && set.len() < target {
        let need = target - set.len();
        // Evenly space additional sample points between 1 and tip-1.
        for i in 1..=need {
            let h = ((tip as u64) * (i as u64) / ((need + 1) as u64)) as u32;
            let h = h.clamp(0, tip);
            set.insert(h);
        }
    }

    // If still short (very small tip), just take every height.
    if set.len() < target && tip > 0 {
        for h in 0..=tip {
            set.insert(h);
            if set.len() >= target {
                break;
            }
        }
    }

    set.into_iter().collect()
}

/// Compare one local block to dashd RPC data (pure, testable).
pub fn compare_height(
    height: u32,
    local_hash_display: Option<&str>,
    local_txids_display: &[String],
    remote_hash: &str,
    remote_txids: &[String],
) -> Option<VerifyMismatch> {
    let Some(local_hash) = local_hash_display else {
        return Some(VerifyMismatch {
            height,
            detail: format!("local block missing; dashd has {remote_hash}"),
        });
    };

    if local_hash != remote_hash {
        return Some(VerifyMismatch {
            height,
            detail: format!("hash mismatch: local={local_hash} dashd={remote_hash}"),
        });
    }

    if local_txids_display.len() != remote_txids.len() {
        return Some(VerifyMismatch {
            height,
            detail: format!(
                "tx count mismatch: local={} dashd={}",
                local_txids_display.len(),
                remote_txids.len()
            ),
        });
    }

    for (i, (local_txid, remote_txid)) in local_txids_display
        .iter()
        .zip(remote_txids.iter())
        .enumerate()
    {
        if local_txid != remote_txid {
            return Some(VerifyMismatch {
                height,
                detail: format!("txid[{i}] mismatch: local={local_txid} dashd={remote_txid}"),
            });
        }
    }

    None
}

/// Run verification of the local DB against dashd.
pub async fn verify_db(
    dbdir: &Path,
    rpc_url: &str,
    rpc_user: &str,
    rpc_password: &str,
    sample_count: u32,
    extra_heights: &[u32],
) -> Result<VerifyReport> {
    let db = DainoDB::open(dbdir)?;
    let rpc = DashdRpc::new(rpc_url, rpc_user, rpc_password);

    println!("Connecting to dashd at {rpc_url} ...");
    let info = rpc
        .get_blockchain_info()
        .await
        .context("Failed to connect to dashd")?;

    let local_tip = db.tip_height()?.unwrap_or(0);
    let dashd_tip = info.blocks as u32;

    println!(
        "Local tip: {local_tip}, dashd tip: {dashd_tip} ({})",
        info.chain
    );

    if db.get_meta()?.is_none() {
        bail!("Local database is empty; index blocks first");
    }

    // Only verify heights we have locally.
    let max_height = local_tip.min(dashd_tip);
    if local_tip != dashd_tip {
        println!("NOTE: tips differ; verifying shared range 0..={max_height}");
    }

    let heights = select_heights(max_height, sample_count, extra_heights);
    println!(
        "Checking {} height(s): {}{}",
        heights.len(),
        heights
            .iter()
            .take(12)
            .map(|h| h.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        if heights.len() > 12 { ", ..." } else { "" }
    );

    let mut report = VerifyReport {
        checked: 0,
        mismatches: Vec::new(),
    };

    for height in heights {
        report.checked += 1;

        let local = db
            .get_block_by_height(height)
            .with_context(|| format!("Failed to read local block at {height}"))?;

        let local_hash = local.as_ref().map(|b| hash_to_display(&b.hash));
        let local_txids = db.get_block_txids(height)?;
        let local_txids_display: Vec<String> = local_txids.iter().map(hash_to_display).collect();

        let remote_hash = rpc
            .get_block_hash(height as u64)
            .await
            .with_context(|| format!("getblockhash {height}"))?;

        let remote_block = rpc
            .get_block(&remote_hash)
            .await
            .with_context(|| format!("getblock {remote_hash}"))?;

        if let Some(m) = compare_height(
            height,
            local_hash.as_deref(),
            &local_txids_display,
            &remote_hash,
            &remote_block.tx,
        ) {
            println!("  FAIL height {height}: {}", m.detail);
            report.mismatches.push(m);
        } else {
            let n = local_txids_display.len();
            println!("  ok   height {height} hash={} txs={n}", &remote_hash[..16]);
        }
    }

    println!();
    if report.ok() {
        println!("Verify passed: {} height(s), 0 mismatches", report.checked);
    } else {
        println!(
            "Verify FAILED: {} height(s) checked, {} mismatch(es)",
            report.checked,
            report.mismatches.len()
        );
    }

    Ok(report)
}

/// CLI entry: run verify and error if mismatches found.
pub async fn run_verify(
    dbdir: &Path,
    rpc_url: &str,
    rpc_user: &str,
    rpc_password: &str,
    sample_count: u32,
    extra_heights: &[u32],
) -> Result<()> {
    let report = verify_db(
        dbdir,
        rpc_url,
        rpc_user,
        rpc_password,
        sample_count,
        extra_heights,
    )
    .await?;

    if !report.ok() {
        bail!(
            "verification failed with {} mismatch(es)",
            report.mismatches.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_heights_includes_ends() {
        let h = select_heights(100, 5, &[]);
        assert_eq!(h.first().copied(), Some(0));
        assert_eq!(h.last().copied(), Some(100));
        assert!(h.len() >= 2);
        assert!(h.len() <= 5 + 2); // extras may expand slightly via spacing
    }

    #[test]
    fn test_select_heights_extras() {
        let h = select_heights(1000, 3, &[42, 9999]);
        assert!(h.contains(&0));
        assert!(h.contains(&42));
        assert!(h.contains(&1000));
        assert!(!h.contains(&9999)); // above tip
    }

    #[test]
    fn test_select_heights_genesis_only() {
        let h = select_heights(0, 10, &[]);
        assert_eq!(h, vec![0]);
    }

    #[test]
    fn test_compare_height_match() {
        let m = compare_height(
            1,
            Some("aa"),
            &["t1".into(), "t2".into()],
            "aa",
            &["t1".into(), "t2".into()],
        );
        assert!(m.is_none());
    }

    #[test]
    fn test_compare_height_hash_mismatch() {
        let m = compare_height(1, Some("aa"), &[], "bb", &[]).unwrap();
        assert_eq!(m.height, 1);
        assert!(m.detail.contains("hash mismatch"));
    }

    #[test]
    fn test_compare_height_tx_count() {
        let m = compare_height(
            2,
            Some("aa"),
            &["t1".into()],
            "aa",
            &["t1".into(), "t2".into()],
        )
        .unwrap();
        assert!(m.detail.contains("tx count"));
    }

    #[test]
    fn test_compare_height_txid_mismatch() {
        let m = compare_height(
            3,
            Some("aa"),
            &["t1".into(), "x".into()],
            "aa",
            &["t1".into(), "t2".into()],
        )
        .unwrap();
        assert!(m.detail.contains("txid[1]"));
    }

    #[test]
    fn test_compare_height_missing_local() {
        let m = compare_height(0, None, &[], "genesis", &[]).unwrap();
        assert!(m.detail.contains("local block missing"));
    }
}
