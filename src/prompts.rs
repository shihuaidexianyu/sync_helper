use anyhow::Result;
use console::style;
use dialoguer::{Confirm, Input, Select};
use std::path::Path;

use crate::config::save_config;
use crate::models::{
    AppConfig, FilterMode, ServerProfile, SyncPreset, TransferMode, TransferPaths,
    default_filter_mode, default_ssh_port, default_sync_preset,
};
use crate::planner::{
    SyncPlan, compare_mode_label, delete_mode_label, filter_mode_label, path_mode_label,
    preview_route, source_kind_label, sync_preset_description, sync_preset_label,
    transfer_mode_label,
};

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
        push_defaults: None,
        pull_defaults: None,
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
) -> Result<(u16, TransferPaths)> {
    let local_prompt = match mode {
        TransferMode::Push => "Enter local source path (or drag it here)",
        TransferMode::Pull => "Enter local destination directory",
    };
    let remote_prompt = match mode {
        TransferMode::Push => "Remote target path",
        TransferMode::Pull => "Remote source path",
    };
    let port = prompt_port(default_port)?;
    let remote_dir = prompt_non_empty_with_optional_default(
        remote_prompt,
        defaults.map(|paths| paths.remote_dir.as_str()),
    )?;
    let local_path = prompt_path_with_optional_default(
        local_prompt,
        defaults.map(|paths| paths.local_path.as_str()),
    )?;
    let sync_preset = prompt_sync_preset(
        mode,
        defaults
            .map(|paths| paths.sync_preset)
            .unwrap_or_else(default_sync_preset),
    )?;
    let filter_mode = prompt_transfer_filter_mode(
        defaults
            .map(|paths| paths.effective_filter_mode())
            .unwrap_or_else(default_filter_mode),
    )?;
    let dry_run = prompt_dry_run(defaults.is_some_and(|paths| paths.dry_run))?;
    Ok((
        port,
        TransferPaths {
            local_path,
            remote_dir,
            sync_preset,
            filter_mode: Some(filter_mode),
            dry_run,
            use_gitignore: false,
            ignore_git_dir: true,
        },
    ))
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
        &preview_route(mode, &last.local_path, &last.remote_dir),
    );
    print_summary_line("Strategy", sync_preset_label(last.sync_preset));
    print_summary_line("Filter", filter_mode_label(last.effective_filter_mode()));
    print_summary_line("Dry run", bool_label(last.dry_run));
    Confirm::new()
        .with_prompt(prompt_text("Reuse these settings?"))
        .default(true)
        .interact()
        .map_err(Into::into)
}

pub fn print_transfer_summary(server: &ServerProfile, plan: &SyncPlan) {
    println!();
    println!("{}", style("Transfer summary").cyan().bold());
    print_summary_line("Server", &server_label(server));
    print_summary_line("Mode", transfer_mode_label(plan.mode));
    print_summary_line("Port", &plan.port.to_string());
    print_summary_line("Local", &plan.local_path);
    print_summary_line("Remote input", &plan.remote_input);
    print_summary_line("Resolved remote", &plan.resolved_remote_path);
    print_summary_line(
        "Route",
        &preview_route(plan.mode, &plan.local_path, &plan.resolved_remote_path),
    );
    print_summary_line("Source kind", source_kind_label(plan.source_kind));
    print_summary_line("Path mode", path_mode_label(plan.path_mode));
    print_summary_line("Strategy", sync_preset_label(plan.policy.preset));
    print_summary_line("Policy", sync_preset_description(plan.policy.preset));
    print_summary_line("Compare", compare_mode_label(plan.policy.compare_mode));
    print_summary_line("Delete extra", delete_mode_label(plan.policy.delete_mode));
    print_summary_line("Filter", filter_mode_label(plan.filter_mode));
    print_summary_line("Dry run", bool_label(plan.policy.dry_run));

    if let Some(path) = &plan.remote_mkdir_path {
        print_summary_line("Remote mkdir", path);
    }
    if let Some(path) = &plan.local_filter_file {
        print_summary_line("Exclude file", &path.display().to_string());
    }
}

pub fn confirm_start_transfer(plan: &SyncPlan) -> Result<bool> {
    if matches!(
        plan.policy.preset,
        SyncPreset::Mirror
    ) {
        println!(
            "{}",
            style(format!(
                "Mirror mode will delete extra files under {}.",
                plan.resolved_remote_path
            ))
            .yellow()
            .bold()
        );
    }
    if plan.policy.dry_run {
        println!(
            "{}",
            style("Dry run is enabled. No files will be modified.").yellow()
        );
    }
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

fn prompt_sync_preset(mode: TransferMode, default_preset: SyncPreset) -> Result<SyncPreset> {
    let allow_mirror = matches!(mode, TransferMode::Push);
    let mut preset_values = vec![SyncPreset::Fast, SyncPreset::Strict];
    if allow_mirror {
        preset_values.push(SyncPreset::Mirror);
    }

    let items: Vec<String> = preset_values
        .iter()
        .map(|preset| {
            format!(
                "{} {}",
                style(sync_preset_label(*preset)).green().bold(),
                style(sync_preset_description(*preset)).dim()
            )
        })
        .collect();
    let fallback = if allow_mirror || !matches!(default_preset, SyncPreset::Mirror) {
        default_preset
    } else {
        SyncPreset::Strict
    };
    let default_index = preset_values
        .iter()
        .position(|preset| *preset == fallback)
        .unwrap_or(1);

    let selection = Select::new()
        .with_prompt(prompt_text("Sync strategy"))
        .items(&items)
        .default(default_index)
        .interact()?;
    Ok(preset_values[selection])
}

fn prompt_transfer_filter_mode(default_filter_mode: FilterMode) -> Result<FilterMode> {
    let filter_values = [
        FilterMode::None,
        FilterMode::ExcludeGitDir,
        FilterMode::LocalGitignore,
        FilterMode::LocalGitignoreAndGitDir,
    ];
    let items = [
        format!("{}", style("No extra filtering").dim()),
        format!("{}", style("Exclude .git/ only").yellow()),
        format!(
            "{}",
            style("Apply local .gitignore as rsync exclude rules").cyan()
        ),
        format!(
            "{}",
            style("Apply local .gitignore as rsync exclude rules + exclude .git/")
                .green()
        ),
    ];
    let default_index = filter_values
        .iter()
        .position(|mode| *mode == default_filter_mode)
        .unwrap_or(1);
    let selection = Select::new()
        .with_prompt(prompt_text("Transfer filtering"))
        .items(&items)
        .default(default_index)
        .interact()?;
    Ok(filter_values[selection])
}

fn prompt_dry_run(default_dry_run: bool) -> Result<bool> {
    Confirm::new()
        .with_prompt(prompt_text("Preview only (dry run)?"))
        .default(default_dry_run)
        .interact()
        .map_err(Into::into)
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

fn bool_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
