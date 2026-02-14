//! The `serve` subcommand -- start the REST API server.

use anyhow::Result;
use std::path::Path;

use daino_serve::RpcConfig;

pub async fn run_server(
    dbdir: &Path,
    listen: &str,
    rpc_config: Option<RpcConfig>,
) -> Result<()> {
    if !dbdir.exists() {
        anyhow::bail!(
            "Database not found at {:?}\nRun `dainod index` first.",
            dbdir
        );
    }

    daino_serve::start_server(dbdir, listen, rpc_config).await
}
