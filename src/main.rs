mod config;
mod models;
mod prompts;
mod transfer;

use anyhow::Result;
use console::style;

use crate::config::{config_file_path, load_config, save_config};
use crate::models::{TransferMode, TransferPaths};
use crate::prompts::{
    confirm_start_transfer, create_server_wizard, print_transfer_summary,
    prompt_reuse_last_settings, prompt_transfer_inputs, prompt_transfer_mode, select_server,
};
use crate::transfer::{ensure_rsync_available, run_rsync};

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
    let saved_paths = server.active_paths().cloned();

    let (port, local_path, remote_dir, use_gitignore, ignore_git_dir) =
        if let Some(last) = saved_paths.as_ref() {
            if prompt_reuse_last_settings(mode, &server, last)? {
                (
                    server.port,
                    last.local_path.clone(),
                    last.remote_dir.clone(),
                    last.use_gitignore,
                    last.ignore_git_dir,
                )
            } else {
                prompt_transfer_inputs(mode, server.port, saved_paths.as_ref())?
            }
        } else {
            prompt_transfer_inputs(mode, server.port, saved_paths.as_ref())?
        };

    print_transfer_summary(
        &server,
        mode,
        port,
        &local_path,
        &remote_dir,
        use_gitignore,
        ignore_git_dir,
    );
    if !confirm_start_transfer()? {
        println!("{}", style("Canceled.").yellow());
        return Ok(());
    }

    server.port = port;
    let transfer_paths = TransferPaths {
        local_path: local_path.clone(),
        remote_dir: remote_dir.clone(),
        use_gitignore,
        ignore_git_dir,
    };
    server.shared_paths = Some(transfer_paths);
    server.push_paths = None;
    server.pull_paths = None;
    config.servers[server_index] = server.clone();
    save_config(&config_path, &config)?;

    run_rsync(
        &server,
        mode,
        port,
        &local_path,
        &remote_dir,
        use_gitignore,
        ignore_git_dir,
    )?;

    Ok(())
}
