use anyhow::{Context, Result};
use dialoguer::Confirm;
use directories::ProjectDirs;
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::AppConfig;

pub fn config_file_path() -> Result<PathBuf> {
    let proj_dirs =
        ProjectDirs::from("com", "rup", "rup").context("Failed to resolve config directory")?;
    Ok(proj_dirs.config_dir().join("config.toml"))
}

pub fn load_config(path: &Path) -> Result<AppConfig> {
    let content = match fs::read_to_string(path) {
        Ok(data) => data,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(AppConfig::default()),
        Err(err) => return Err(err).context("Failed to read config file"),
    };

    match toml::from_str::<AppConfig>(&content) {
        Ok(mut config) => {
            migrate_legacy_paths(&mut config);
            Ok(config)
        }
        Err(err) => {
            let reset = Confirm::new()
                .with_prompt("Config file is corrupted. Reset it?")
                .default(false)
                .interact()?;
            if reset {
                Ok(AppConfig::default())
            } else {
                Err(err).context("Failed to parse config file")
            }
        }
    }
}

pub fn save_config(path: &Path, config: &AppConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create config directory")?;
    }
    let content = toml::to_string_pretty(config).context("Failed to serialize config")?;
    fs::write(path, content).context("Failed to write config file")?;
    Ok(())
}

fn migrate_legacy_paths(config: &mut AppConfig) {
    for server in &mut config.servers {
        if server.shared_paths.is_none() {
            server.shared_paths = server
                .push_paths
                .clone()
                .or_else(|| server.pull_paths.clone());
        }
    }
}
