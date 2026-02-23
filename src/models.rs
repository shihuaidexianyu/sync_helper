use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerProfile {
    pub user: String,
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    #[serde(default)]
    pub shared_paths: Option<TransferPaths>,
    #[serde(default, skip_serializing)]
    pub push_paths: Option<TransferPaths>,
    #[serde(default, skip_serializing)]
    pub pull_paths: Option<TransferPaths>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransferPaths {
    pub local_path: String,
    pub remote_dir: String,
    #[serde(default)]
    pub use_gitignore: bool,
    #[serde(default = "default_ignore_git_dir")]
    pub ignore_git_dir: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum TransferMode {
    Push,
    Pull,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub servers: Vec<ServerProfile>,
}

pub fn default_ssh_port() -> u16 {
    22
}

pub fn default_ignore_git_dir() -> bool {
    true
}

impl ServerProfile {
    pub fn active_paths(&self) -> Option<&TransferPaths> {
        self.shared_paths
            .as_ref()
            .or(self.push_paths.as_ref())
            .or(self.pull_paths.as_ref())
    }
}
