mod config;
mod executor;
mod models;
mod planner;
mod prompts;

use anyhow::Result;
use console::style;

use crate::config::{config_file_path, load_config, save_config};
use crate::executor::{ensure_rsync_available, run_sync_plan};
use crate::models::TransferMode;
use crate::planner::build_sync_plan;
use crate::prompts::{
    confirm_start_transfer, create_server_wizard, print_transfer_summary,
    prompt_reuse_last_settings, prompt_transfer_inputs, prompt_transfer_mode, select_server,
};

fn main() -> Result<()> {
    ensure_rsync_available()?;

    let config_path = config_file_path()?;
    let mut config = load_config(&config_path)?;

    if config.servers.is_empty() {
        let server = create_server_wizard()?;
        config.servers.push(server);
        save_config(&config_path, &config)?;
    }

    let server_index = select_server(&mut config, &config_path)?;
    let mut server = config.servers[server_index].clone();
    let mode = prompt_transfer_mode(TransferMode::Push)?;
    let saved_paths = server.defaults_for_mode(mode).cloned();

    let (port, transfer_paths) = if let Some(last) = saved_paths.as_ref() {
        if prompt_reuse_last_settings(mode, &server, last)? {
            (server.port, last.clone())
        } else {
            prompt_transfer_inputs(mode, server.port, saved_paths.as_ref())?
        }
    } else {
        prompt_transfer_inputs(mode, server.port, saved_paths.as_ref())?
    };

    let plan = build_sync_plan(&server, mode, port, &transfer_paths)?;

    print_transfer_summary(&server, &plan);
    if !confirm_start_transfer(&plan)? {
        println!("{}", style("Canceled.").yellow());
        return Ok(());
    }

    server.port = port;
    server.set_defaults_for_mode(mode, transfer_paths);
    config.servers[server_index] = server.clone();
    save_config(&config_path, &config)?;

    run_sync_plan(&server, &plan)?;

    Ok(())
}
