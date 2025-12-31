use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub dashd: DashdConfig,
    pub indexer: IndexerConfig,
    pub api: ApiConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashdConfig {
    pub zmq_endpoint: String,
    pub rpc_url: String,
    pub rpc_user: String,
    pub rpc_password: String,
    pub datadir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerConfig {
    pub datadir: String,
    pub bootstrap_from_files: bool,
    pub indexes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub grpc_listen: String,
    pub jsonrpc_listen: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String, // "json" or "pretty"
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dashd: DashdConfig {
                zmq_endpoint: "tcp://127.0.0.1:28332".to_string(),
                rpc_url: "http://127.0.0.1:9998".to_string(),
                rpc_user: "user".to_string(),
                rpc_password: "pass".to_string(),
                datadir: None,
            },
            indexer: IndexerConfig {
                datadir: "./index-data".to_string(),
                bootstrap_from_files: true,
                indexes: vec!["tx".to_string()],
            },
            api: ApiConfig {
                grpc_listen: "127.0.0.1:50051".to_string(),
                jsonrpc_listen: Some("127.0.0.1:3000".to_string()),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "pretty".to_string(),
            },
        }
    }
}
