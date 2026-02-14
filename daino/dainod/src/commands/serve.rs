//! The `serve` subcommand -- start the REST API server.

use anyhow::Result;
use std::path::Path;

pub fn run_server(dbdir: &Path, listen: &str) -> Result<()> {
    if !dbdir.exists() {
        anyhow::bail!(
            "Database not found at {:?}\nRun `dainod index` first.",
            dbdir
        );
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(daino_serve::start_server(dbdir, listen))
}
