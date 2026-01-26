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
    user: String,
    host: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum TransferMode {
    Push,
    Pull,
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
    let mode = prompt_transfer_mode(TransferMode::Push)?;
    let port = prompt_port()?;
    let remote_dir = prompt_non_empty("Remote directory")?;
    let local_path = match mode {
        TransferMode::Push => prompt_path("Enter local source path (or drag it here)")?,
        TransferMode::Pull => prompt_path("Enter local destination directory")?,
    };
    run_rsync(&server, mode, port, &local_path, &remote_dir)?;

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
    loop {
        let mut items: Vec<String> = config
            .servers
            .iter()
            .map(server_label)
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
            return Ok(server);
        }

        let existing = config.servers[selection].clone();
        let label = server_label(&existing);
        let actions = ["Use this server", "Edit", "Delete", "Back"];
        let action = Select::new()
            .with_prompt(format!("{} selected", label))
            .items(&actions)
            .default(0)
            .interact()?;

        match action {
            0 => return Ok(existing),
            1 => {
                let updated = edit_server_wizard(&existing)?;
                config.servers[selection] = updated.clone();
                save_config(path, config)?;
                return Ok(updated);
            }
            2 => {
                let confirmed = Confirm::new()
                    .with_prompt(format!("Delete {}?", label))
                    .default(false)
                    .interact()?;
                if confirmed {
                    config.servers.remove(selection);
                    save_config(path, config)?;
                    if config.servers.is_empty() {
                        let server = create_server_wizard()?;
                        config.servers.push(server.clone());
                        save_config(path, config)?;
                        return Ok(server);
                    }
                }
            }
            _ => {}
        }
    }
}

fn create_server_wizard() -> Result<ServerProfile> {
    let user = prompt_non_empty("User")?;
    let host = prompt_non_empty("Host")?;

    Ok(ServerProfile {
        user,
        host,
    })
}

fn edit_server_wizard(existing: &ServerProfile) -> Result<ServerProfile> {
    let user = prompt_with_default("User", &existing.user)?;
    let host = prompt_with_default("Host", &existing.host)?;

    Ok(ServerProfile {
        user,
        host,
    })
}

fn server_label(server: &ServerProfile) -> String {
    format!("{}@{}", server.user, server.host)
}

fn prompt_transfer_mode(default_mode: TransferMode) -> Result<TransferMode> {
    let items = ["push (local → remote)", "pull (local ← remote)"];
    let default_index = match default_mode {
        TransferMode::Push => 0,
        TransferMode::Pull => 1,
    };
    let selection = Select::new()
        .with_prompt("Transfer mode")
        .items(&items)
        .default(default_index)
        .interact()?;
    Ok(if selection == 0 {
        TransferMode::Push
    } else {
        TransferMode::Pull
    })
}

fn prompt_port() -> Result<u16> {
    let port: u16 = Input::new()
        .with_prompt("Port")
        .default(default_ssh_port())
        .interact_text()?;
    Ok(port)
}

fn default_ssh_port() -> u16 {
    22
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

fn prompt_with_default(label: &str, default: &str) -> Result<String> {
    loop {
        let value: String = Input::new()
            .with_prompt(label)
            .default(default.to_string())
            .interact_text()?;
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
        println!("{}", style("Value cannot be empty.").yellow());
    }
}

fn prompt_path(prompt: &str) -> Result<String> {
    loop {
        let raw: String = Input::new()
            .with_prompt(prompt)
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

fn run_rsync(
    server: &ServerProfile,
    mode: TransferMode,
    port: u16,
    local_path: &str,
    remote_dir: &str,
) -> Result<()> {
    let (source, destination) = match mode {
        TransferMode::Push => {
            let destination = format!("{}@{}:{}", server.user, server.host, remote_dir);
            (local_path.to_string(), destination)
        }
        TransferMode::Pull => {
            let source = format!("{}@{}:{}", server.user, server.host, remote_dir);
            (source, local_path.to_string())
        }
    };

    let status = Command::new("rsync")
        .arg("-avzP")
        .arg("-e")
    .arg(format!("ssh -p {}", port))
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
