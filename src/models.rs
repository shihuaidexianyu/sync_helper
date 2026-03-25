use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerProfile {
    pub user: String,
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_defaults: Option<TransferPaths>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_defaults: Option<TransferPaths>,
    #[serde(default, skip_serializing)]
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
    #[serde(default = "default_sync_preset")]
    pub sync_preset: SyncPreset,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_mode: Option<FilterMode>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default, skip_serializing)]
    pub use_gitignore: bool,
    #[serde(default = "default_ignore_git_dir", skip_serializing)]
    pub ignore_git_dir: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferMode {
    Push,
    Pull,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncPreset {
    Fast,
    Strict,
    Mirror,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode {
    None,
    LocalGitignore,
    ExcludeGitDir,
    LocalGitignoreAndGitDir,
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

pub fn default_sync_preset() -> SyncPreset {
    SyncPreset::Strict
}

pub fn default_filter_mode() -> FilterMode {
    FilterMode::ExcludeGitDir
}

impl ServerProfile {
    pub fn defaults_for_mode(&self, mode: TransferMode) -> Option<&TransferPaths> {
        match mode {
            TransferMode::Push => self.push_defaults.as_ref(),
            TransferMode::Pull => self.pull_defaults.as_ref(),
        }
    }

    pub fn set_defaults_for_mode(&mut self, mode: TransferMode, defaults: TransferPaths) {
        match mode {
            TransferMode::Push => self.push_defaults = Some(defaults),
            TransferMode::Pull => self.pull_defaults = Some(defaults),
        }
    }
}

impl TransferPaths {
    pub fn effective_filter_mode(&self) -> FilterMode {
        self.filter_mode.unwrap_or_else(default_filter_mode)
    }
}
