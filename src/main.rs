use anyhow::{Context, Result, bail};
use console::style;
use dialoguer::{Confirm, Input, Select};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ServerProfile {
    alias: String,
    user: String,
    host: String,
    target_dir: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct AppConfig {
    servers: Vec<ServerProfile>,
}

fn main() -> Result<()> {
    ensure_rsync_available()?;

    let config_path = config_file_path()?;
    let mut config = load_config(&config_path)?;

    if config.servers.is_empty() {
        let server = create_server_wizard()?;
        config.servers.push(server);
        save_config(&config_path, &config)?;
    }

    let server = select_server(&mut config, &config_path)?;
    let source = prompt_source_path()?;
    run_rsync(&server, &source)?;

    Ok(())
}

fn ensure_rsync_available() -> Result<()> {
    let status = Command::new("rsync")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match status {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            bail!("rsync not found in PATH. Please install rsync first.")
        }
        Err(err) => Err(err).context("Failed to check rsync availability"),
    }
}

fn config_file_path() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "rup", "rup")
        .context("Failed to resolve config directory")?;
    Ok(proj_dirs.config_dir().join("config.toml"))
}

fn load_config(path: &Path) -> Result<AppConfig> {
    let content = match fs::read_to_string(path) {
        Ok(data) => data,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(AppConfig::default()),
        Err(err) => return Err(err).context("Failed to read config file"),
    };

    match toml::from_str::<AppConfig>(&content) {
        Ok(config) => Ok(config),
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

fn save_config(path: &Path, config: &AppConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create config directory")?;
    }
    let content = toml::to_string_pretty(config).context("Failed to serialize config")?;
    fs::write(path, content).context("Failed to write config file")?;
    Ok(())
}

fn select_server(config: &mut AppConfig, path: &Path) -> Result<ServerProfile> {
    let mut items: Vec<String> = config
        .servers
        .iter()
        .map(|s| s.alias.clone())
        .collect();
    items.push(String::from("+ Add new server"));

    let selection = Select::new()
        .with_prompt("Select a server")
        .items(&items)
        .default(0)
        .interact()?;

    if selection == items.len() - 1 {
        let server = create_server_wizard()?;
        config.servers.push(server.clone());
        save_config(path, config)?;
        Ok(server)
    } else {
        Ok(config.servers[selection].clone())
    }
}

fn create_server_wizard() -> Result<ServerProfile> {
    let alias = prompt_non_empty("Alias")?;
    let user = prompt_non_empty("User")?;
    let host = prompt_non_empty("Host")?;
    let target_dir = prompt_non_empty("Target directory")?;

    Ok(ServerProfile {
        alias,
        user,
        host,
        target_dir,
    })
}

fn prompt_non_empty(label: &str) -> Result<String> {
    loop {
        let value: String = Input::new().with_prompt(label).interact_text()?;
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
        println!("{}", style("Value cannot be empty.").yellow());
    }
}

fn prompt_source_path() -> Result<String> {
    loop {
        let raw: String = Input::new()
            .with_prompt("Enter a file/folder path (or drag it here)")
            .interact_text()?;
        let cleaned = sanitize_path(&raw);
        if cleaned.is_empty() {
            println!("{}", style("Path cannot be empty.").yellow());
            continue;
        }
        return Ok(cleaned);
    }
}

fn sanitize_path(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.len() >= 2 {
        let first = trimmed.as_bytes()[0];
        let last = trimmed.as_bytes()[trimmed.len() - 1];
        if (first == b'\'' && last == b'\'') || (first == b'"' && last == b'"') {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

fn run_rsync(server: &ServerProfile, source: &str) -> Result<()> {
    let destination = format!("{}@{}:{}", server.user, server.host, server.target_dir);

    let status = Command::new("rsync")
        .arg("-avzP")
        .arg(source)
        .arg(destination)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to start rsync")?;

    if status.success() {
        Ok(())
    } else {
        println!("{}", style("Transfer failed. Check SSH configuration or network.").red());
        Err(anyhow::anyhow!("rsync exited with status {}", status))
    }
}
