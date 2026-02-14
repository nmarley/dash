//! The `compact` subcommand -- compact the LMDB database to reclaim dead pages.

use std::path::Path;

use anyhow::{Context, Result};

use daino_state::db::DainoDB;

/// Human-readable file size formatting.
fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Compact the database by copying with LMDB's MDB_CP_COMPACT flag,
/// then replacing the original data file.
pub fn compact_db(dbdir: &Path) -> Result<()> {
    if !dbdir.exists() {
        anyhow::bail!(
            "Database not found at {:?}\n\
             Run `dainod index` first to create it.",
            dbdir
        );
    }

    let db = DainoDB::open(dbdir)?;

    let size_before = db.real_disk_size()?;
    println!("Database size before: {}", human_size(size_before));
    println!("Compacting...");

    // Compact to a temp file alongside the DB directory
    let temp_path = dbdir.join("data.mdb.compact");
    if temp_path.exists() {
        std::fs::remove_file(&temp_path)
            .with_context(|| format!("Failed to remove stale temp file: {:?}", temp_path))?;
    }

    db.compact(&temp_path)?;

    // Get compacted size before replacing
    let compacted_size = std::fs::metadata(&temp_path)?.len();

    // Replace original data.mdb with the compacted copy
    let data_path = dbdir.join("data.mdb");
    std::fs::rename(&temp_path, &data_path)
        .with_context(|| "Failed to replace data.mdb with compacted copy")?;

    println!("Database size after:  {}", human_size(compacted_size));

    if size_before > 0 {
        let saved = size_before.saturating_sub(compacted_size);
        let pct = (saved as f64 / size_before as f64) * 100.0;
        println!("Saved {} ({:.1}% reduction)", human_size(saved), pct,);
    }

    Ok(())
}
