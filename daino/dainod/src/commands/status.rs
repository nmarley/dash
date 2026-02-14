//! The `status` subcommand -- show database status.

use anyhow::Result;
use std::path::Path;

use daino_state::db::DainoDB;

pub fn show_status(dbdir: &Path) -> Result<()> {
    if !dbdir.exists() {
        println!("Database not found at {:?}", dbdir);
        println!("Run `dainod index` first to create it.");
        return Ok(());
    }

    let db = DainoDB::open(dbdir)?;

    match db.get_meta()? {
        Some(meta) => {
            println!("Daino database status:");
            println!("  Tip height:  {}", meta.tip_height);
            println!(
                "  Tip hash:    {}",
                librustdash::hash::hash_to_display(&meta.tip_hash)
            );
            println!("  Blocks:      {}", meta.block_count);
            println!("  Transactions: {}", meta.tx_count);
        }
        None => {
            println!("Database exists but is empty.");
            println!("Run `dainod index` to populate it.");
        }
    }

    Ok(())
}
