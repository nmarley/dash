//! The `serve` subcommand -- start the REST API server.

use anyhow::Result;
use std::path::Path;

pub async fn run_server(dbdir: &Path, listen: &str) -> Result<()> {
    if !dbdir.exists() {
        anyhow::bail!(
            "Database not found at {:?}\nRun `dainod index` first.",
            dbdir
        );
    }

    daino_serve::start_server(dbdir, listen).await
}
