mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use daino_core::Network;

#[derive(Parser)]
#[command(name = "dainod")]
#[command(about = "Dash blockchain indexer and block explorer backend")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Network type (mainnet, testnet, regtest)
    #[arg(short, long, default_value = "mainnet", global = true)]
    network: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Read and display blocks or undo data from raw files
    Read {
        /// Path to the file (e.g., blk00000.dat or rev00000.dat)
        file: PathBuf,

        /// Number of blocks/undo records to read
        #[arg(short, long, default_value = "1")]
        count: usize,

        /// Read undo file (rev*.dat) instead of block file
        #[arg(short, long)]
        undo: bool,

        /// Show coinbase message
        #[arg(long)]
        show_coinbase_message: bool,

        /// Show raw block header bytes
        #[arg(long)]
        show_raw_block: bool,
    },

    /// Index block files into the database
    Index {
        /// Directory containing blk*.dat block files
        #[arg(short, long)]
        datadir: PathBuf,

        /// Database directory for indexed data
        #[arg(short = 'D', long, default_value = "daino.db")]
        dbdir: PathBuf,

        /// Maximum number of blocks to index (0 = all)
        #[arg(short, long, default_value = "0")]
        max_blocks: usize,

        /// Number of blocks per LMDB write transaction (higher = faster, more RAM)
        #[arg(short, long, default_value = "500")]
        batch_size: usize,
    },

    /// Show database status
    Status {
        /// Database directory
        #[arg(short = 'D', long, default_value = "daino.db")]
        dbdir: PathBuf,
    },

    /// Start the REST API server
    Serve {
        /// Database directory
        #[arg(short = 'D', long, default_value = "daino.db")]
        dbdir: PathBuf,

        /// Listen address (host:port)
        #[arg(short, long, default_value = "127.0.0.1:3141")]
        listen: String,

        /// dashd JSON-RPC URL (enables live ChainLock/IS/spork/governance queries)
        #[arg(long)]
        rpc_url: Option<String>,

        /// RPC username
        #[arg(long, default_value = "dashrpc")]
        rpc_user: String,

        /// RPC password (required if --rpc-url is set)
        #[arg(long)]
        rpc_password: Option<String>,
    },

    /// Follow a running dashd and continuously index new blocks
    Follow {
        /// Database directory
        #[arg(short = 'D', long, default_value = "daino.db")]
        dbdir: PathBuf,

        /// dashd JSON-RPC URL
        #[arg(long, default_value = "http://127.0.0.1:9998")]
        rpc_url: String,

        /// RPC username
        #[arg(long, default_value = "dashrpc")]
        rpc_user: String,

        /// RPC password
        #[arg(long)]
        rpc_password: String,

        /// Poll interval in seconds (how often to check for new blocks)
        #[arg(long, default_value = "2")]
        poll_interval: u64,
    },
}

fn parse_network(s: &str) -> Result<Network> {
    match s {
        "mainnet" => Ok(Network::Mainnet),
        "testnet" => Ok(Network::Testnet),
        "regtest" => Ok(Network::Regtest),
        _ => anyhow::bail!("Invalid network: {}", s),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let network = parse_network(&cli.network)?;

    match cli.command {
        Commands::Read {
            file,
            count,
            undo,
            show_coinbase_message,
            show_raw_block,
        } => {
            if undo {
                commands::read::read_undo_file(&file, network, count)
            } else {
                commands::read::read_block_file(
                    &file,
                    network,
                    count,
                    show_coinbase_message,
                    show_raw_block,
                )
            }
        }
        Commands::Index {
            datadir,
            dbdir,
            max_blocks,
            batch_size,
        } => commands::index::index_blocks(&datadir, &dbdir, network, max_blocks, batch_size),
        Commands::Status { dbdir } => commands::status::show_status(&dbdir),
        Commands::Serve {
            dbdir,
            listen,
            rpc_url,
            rpc_user,
            rpc_password,
        } => {
            let rpc_config = rpc_url.map(|url| daino_serve::RpcConfig {
                url,
                user: rpc_user,
                password: rpc_password.unwrap_or_default(),
            });
            commands::serve::run_server(&dbdir, &listen, rpc_config).await
        }
        Commands::Follow {
            dbdir,
            rpc_url,
            rpc_user,
            rpc_password,
            poll_interval,
        } => {
            commands::follow::follow_dashd(
                &dbdir,
                &rpc_url,
                &rpc_user,
                &rpc_password,
                poll_interval,
            )
            .await
        }
    }
}
