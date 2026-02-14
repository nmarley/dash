//! The `status` subcommand -- show database status.

use anyhow::Result;
use std::path::Path;

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

pub fn show_status(dbdir: &Path) -> Result<()> {
    if !dbdir.exists() {
        println!("Database not found at {:?}", dbdir);
        println!("Run `dainod index` first to create it.");
        return Ok(());
    }

    let db = DainoDB::open(dbdir)?;

    match db.get_meta()? {
        Some(meta) => {
            let db_size = db.real_disk_size()?;
            println!("Daino database status:");
            println!("  Tip height:   {}", meta.tip_height);
            println!(
                "  Tip hash:     {}",
                librustdash::hash::hash_to_display(&meta.tip_hash)
            );
            println!("  Blocks:       {}", meta.block_count);
            println!("  Transactions: {}", meta.tx_count);
            println!("  DB size:      {}", human_size(db_size));
        }
        None => {
            println!("Database exists but is empty.");
            println!("Run `dainod index` to populate it.");
        }
    }

    Ok(())
}
