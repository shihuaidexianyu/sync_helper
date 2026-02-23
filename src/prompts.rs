use anyhow::Result;
use console::style;
use dialoguer::{Confirm, Input, Select};
use std::path::Path;

use crate::config::save_config;
use crate::models::{
    AppConfig, ServerProfile, TransferMode, TransferPaths, default_ignore_git_dir, default_ssh_port,
};
use crate::transfer::{filter_mode_label, transfer_mode_label, transfer_route_preview};

pub fn select_server(config: &mut AppConfig, path: &Path) -> Result<usize> {
    loop {
        println!();
        println!("{}", style("Server selection").cyan().bold());
        println!(
            "  {}",
            style("Choose a server and press Enter to continue.").dim()
        );
        println!(
            "  {}",
            style("+ Add new server: create a new profile.").yellow()
        );
        println!(
            "  {}",
            style("Manage servers: edit/delete existing profiles.").yellow()
        );

        let mut items: Vec<String> = config
            .servers
            .iter()
            .map(|server| format!("{}", style(server_label(server)).blue()))
            .collect();
        items.push(format!("{}", style("+ Add new server").green().bold()));
        items.push(format!(
            "{}",
            style("Manage servers (edit/delete)").yellow().bold()
        ));

        let selection = Select::new()
            .with_prompt(prompt_text("Select a server"))
            .items(&items)
            .default(0)
            .interact()?;

        let add_index = items.len() - 2;
        let manage_index = items.len() - 1;

        if selection == add_index {
            let server = create_server_wizard()?;
            config.servers.push(server);
            save_config(path, config)?;
            return Ok(config.servers.len() - 1);
        }

        if selection == manage_index {
            manage_servers(config, path)?;
            continue;
        }

        return Ok(selection);
    }
}

pub fn create_server_wizard() -> Result<ServerProfile> {
    let user = prompt_non_empty("User")?;
    let host = prompt_non_empty("Host")?;

    Ok(ServerProfile {
        user,
        host,
        port: default_ssh_port(),
        shared_paths: None,
        push_paths: None,
        pull_paths: None,
    })
}

pub fn server_label(server: &ServerProfile) -> String {
    format!("{}@{}", server.user, server.host)
}

pub fn prompt_transfer_mode(default_mode: TransferMode) -> Result<TransferMode> {
    let items = [
        format!("{}", style("push (local -> remote)").green()),
        format!("{}", style("pull (local <- remote)").magenta()),
    ];
    let default_index = match default_mode {
        TransferMode::Push => 0,
        TransferMode::Pull => 1,
    };
    let selection = Select::new()
        .with_prompt(prompt_text("Transfer mode"))
        .items(&items)
        .default(default_index)
        .interact()?;
    Ok(if selection == 0 {
        TransferMode::Push
    } else {
        TransferMode::Pull
    })
}

pub fn prompt_transfer_inputs(
    mode: TransferMode,
    default_port: u16,
    defaults: Option<&TransferPaths>,
) -> Result<(u16, String, String, bool, bool)> {
    let local_prompt = match mode {
        TransferMode::Push => "Enter local source path (or drag it here)",
        TransferMode::Pull => "Enter local destination directory",
    };
    let port = prompt_port(default_port)?;
    let remote_dir = prompt_non_empty_with_optional_default(
        "Remote directory",
        defaults.map(|paths| paths.remote_dir.as_str()),
    )?;
    let local_path = prompt_path_with_optional_default(
        local_prompt,
        defaults.map(|paths| paths.local_path.as_str()),
    )?;
    let (use_gitignore, ignore_git_dir) = prompt_transfer_filters(
        mode,
        defaults.is_some_and(|paths| paths.use_gitignore),
        defaults
            .map(|paths| paths.ignore_git_dir)
            .unwrap_or_else(default_ignore_git_dir),
    )?;
    Ok((port, local_path, remote_dir, use_gitignore, ignore_git_dir))
}

pub fn prompt_reuse_last_settings(
    mode: TransferMode,
    server: &ServerProfile,
    last: &TransferPaths,
) -> Result<bool> {
    println!();
    println!("{}", style("Last saved settings").cyan().bold());
    print_summary_line("Server", &server_label(server));
    print_summary_line("Mode", transfer_mode_label(mode));
    print_summary_line("Port", &server.port.to_string());
    print_summary_line(
        "Route",
        &transfer_route_preview(mode, &last.local_path, &last.remote_dir),
    );
    print_summary_line(
        "Filter",
        filter_mode_label(mode, last.use_gitignore, last.ignore_git_dir),
    );
    Confirm::new()
        .with_prompt(prompt_text("Reuse these settings?"))
        .default(true)
        .interact()
        .map_err(Into::into)
}

pub fn print_transfer_summary(
    server: &ServerProfile,
    mode: TransferMode,
    port: u16,
    local_path: &str,
    remote_dir: &str,
    use_gitignore: bool,
    ignore_git_dir: bool,
) {
    println!();
    println!("{}", style("Transfer summary").cyan().bold());
    print_summary_line("Server", &server_label(server));
    print_summary_line("Mode", transfer_mode_label(mode));
    print_summary_line("Port", &port.to_string());
    print_summary_line(
        "Route",
        &transfer_route_preview(mode, local_path, remote_dir),
    );
    print_summary_line(
        "Filters",
        filter_mode_label(mode, use_gitignore, ignore_git_dir),
    );
}

pub fn confirm_start_transfer() -> Result<bool> {
    Confirm::new()
        .with_prompt(prompt_text("Start transfer now?"))
        .default(true)
        .interact()
        .map_err(Into::into)
}

fn manage_servers(config: &mut AppConfig, path: &Path) -> Result<()> {
    loop {
        println!();
        println!("{}", style("Server management").cyan().bold());
        let mut items: Vec<String> = config
            .servers
            .iter()
            .map(|server| format!("{}", style(server_label(server)).blue()))
            .collect();
        items.push(format!("{}", style("Back").yellow()));

        let selection = Select::new()
            .with_prompt(prompt_text("Select a server to edit/delete"))
            .items(&items)
            .default(0)
            .interact()?;

        if selection == items.len() - 1 {
            return Ok(());
        }

        let existing = config.servers[selection].clone();
        let label = server_label(&existing);
        let actions = [
            format!("{}", style("Edit").cyan()),
            format!("{}", style("Delete").red()),
            format!("{}", style("Back").yellow()),
        ];
        let action = Select::new()
            .with_prompt(prompt_text(&format!("Manage {}", label)))
            .items(&actions)
            .default(0)
            .interact()?;

        match action {
            0 => {
                let updated = edit_server_wizard(&existing)?;
                config.servers[selection] = updated;
                save_config(path, config)?;
                println!("{}", style("Server updated.").green());
            }
            1 => {
                let confirmed = Confirm::new()
                    .with_prompt(format!("Delete {}?", label))
                    .default(false)
                    .interact()?;
                if confirmed {
                    config.servers.remove(selection);
                    save_config(path, config)?;
                    println!("{}", style("Server deleted.").green());
                    if config.servers.is_empty() {
                        let server = create_server_wizard()?;
                        config.servers.push(server);
                        save_config(path, config)?;
                        println!("{}", style("Created a new server profile.").green());
                        return Ok(());
                    }
                }
            }
            _ => {}
        }
    }
}

fn edit_server_wizard(existing: &ServerProfile) -> Result<ServerProfile> {
    let user = prompt_with_default("User", &existing.user)?;
    let host = prompt_with_default("Host", &existing.host)?;

    let mut updated = existing.clone();
    updated.user = user;
    updated.host = host;
    Ok(updated)
}

fn prompt_port(default_port: u16) -> Result<u16> {
    let port: u16 = Input::new()
        .with_prompt(prompt_text("Port"))
        .default(default_port)
        .interact_text()?;
    Ok(port)
}

fn prompt_non_empty(label: &str) -> Result<String> {
    loop {
        let value: String = Input::new()
            .with_prompt(prompt_text(label))
            .interact_text()?;
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
            .with_prompt(prompt_text(label))
            .default(default.to_string())
            .interact_text()?;
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
        println!("{}", style("Value cannot be empty.").yellow());
    }
}

fn prompt_non_empty_with_optional_default(label: &str, default: Option<&str>) -> Result<String> {
    match default {
        Some(value) => prompt_with_default(label, value),
        None => prompt_non_empty(label),
    }
}

fn prompt_path_with_optional_default(prompt: &str, default: Option<&str>) -> Result<String> {
    loop {
        let raw: String = match default {
            Some(value) => Input::new()
                .with_prompt(prompt_text(prompt))
                .default(value.to_string())
                .interact_text()?,
            None => Input::new()
                .with_prompt(prompt_text(prompt))
                .interact_text()?,
        };
        let cleaned = sanitize_path(&raw);
        if cleaned.is_empty() {
            println!("{}", style("Path cannot be empty.").yellow());
            continue;
        }
        return Ok(cleaned);
    }
}

fn prompt_transfer_filters(
    _mode: TransferMode,
    default_use_gitignore: bool,
    default_ignore_git_dir: bool,
) -> Result<(bool, bool)> {
    let items = [
        format!("{}", style("No extra filtering").dim()),
        format!("{}", style("Apply local .gitignore rules").cyan()),
        format!("{}", style("Exclude .git/ only").yellow()),
        format!(
            "{}",
            style("Apply local .gitignore + exclude .git/").green()
        ),
    ];
    let default_index = match (default_use_gitignore, default_ignore_git_dir) {
        (false, false) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (true, true) => 3,
    };
    let selection = Select::new()
        .with_prompt(prompt_text("Transfer filtering"))
        .items(&items)
        .default(default_index)
        .interact()?;
    Ok(match selection {
        0 => (false, false),
        1 => (true, false),
        2 => (false, true),
        _ => (true, true),
    })
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

fn prompt_text(text: &str) -> String {
    format!("{}", style(text).cyan().bold())
}

fn print_summary_line(label: &str, value: &str) {
    println!(
        "  {} {}",
        style(format!("{label}:")).blue().bold(),
        style(value).green().bold()
    );
}
