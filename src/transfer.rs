use anyhow::{Context, Result, bail};
use console::style;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::models::{ServerProfile, TransferMode};

pub fn ensure_rsync_available() -> Result<()> {
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

pub fn transfer_mode_label(mode: TransferMode) -> &'static str {
    match mode {
        TransferMode::Push => "push",
        TransferMode::Pull => "pull",
    }
}

pub fn transfer_route_preview(mode: TransferMode, local_path: &str, remote_dir: &str) -> String {
    let remote_sync_path = resolve_remote_sync_path(local_path, remote_dir);
    match mode {
        TransferMode::Push => format!("{} -> {}", local_path, remote_sync_path),
        TransferMode::Pull => format!("{} -> {}", remote_sync_path, local_path),
    }
}

pub fn filter_mode_label(
    _mode: TransferMode,
    use_gitignore: bool,
    ignore_git_dir: bool,
) -> &'static str {
    match (use_gitignore, ignore_git_dir) {
        (false, false) => "none",
        (true, false) => ".gitignore only",
        (false, true) => "exclude .git/ only",
        (true, true) => ".gitignore + exclude .git/",
    }
}

pub fn run_rsync(
    server: &ServerProfile,
    mode: TransferMode,
    port: u16,
    local_path: &str,
    remote_dir: &str,
    use_gitignore: bool,
    ignore_git_dir: bool,
) -> Result<()> {
    let remote_sync_path = resolve_remote_sync_path(local_path, remote_dir);
    let push_source_is_dir = fs::metadata(local_path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false);
    let sync_dir_contents = has_trailing_separator(remote_dir) || push_source_is_dir;

    let local_gitignore = if use_gitignore {
        Some(
            resolve_local_gitignore_path(mode, local_path)
                .context("Local .gitignore not found for the selected local path")?,
        )
    } else {
        None
    };

    let (source, destination) = match mode {
        TransferMode::Push => {
            let destination = format!("{}@{}:{}", server.user, server.host, remote_sync_path);
            let source = if push_source_is_dir {
                ensure_trailing_separator(local_path)
            } else {
                local_path.to_string()
            };
            (source, destination)
        }
        TransferMode::Pull => {
            let remote_source = if sync_dir_contents {
                ensure_trailing_separator(&remote_sync_path)
            } else {
                remote_sync_path.clone()
            };
            let source = format!("{}@{}:{}", server.user, server.host, remote_source);
            (source, local_path.to_string())
        }
    };

    if matches!(mode, TransferMode::Push) {
        ensure_remote_path_for_push(server, port, &remote_sync_path, push_source_is_dir)?;
    }

    let status = run_rsync_standard(
        &source,
        &destination,
        port,
        local_gitignore.as_deref(),
        ignore_git_dir,
    )?;

    if status.success() {
        Ok(())
    } else {
        println!(
            "{}",
            style("Transfer failed. Check SSH configuration or network.").red()
        );
        Err(anyhow::anyhow!("rsync exited with status {}", status))
    }
}

fn run_rsync_standard(
    source: &str,
    destination: &str,
    port: u16,
    local_gitignore: Option<&Path>,
    ignore_git_dir: bool,
) -> Result<std::process::ExitStatus> {
    let mut cmd = Command::new("rsync");
    cmd.arg("-avzP").arg("-e").arg(format!("ssh -p {}", port));
    if let Some(path) = local_gitignore {
        cmd.arg("--exclude-from").arg(path);
    }
    if ignore_git_dir {
        cmd.arg("--exclude=.git/");
    }

    cmd.arg(source)
        .arg(destination)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to start rsync")
}

fn has_trailing_separator(path: &str) -> bool {
    path.ends_with('/') || path.ends_with('\\')
}

fn ensure_remote_path_for_push(
    server: &ServerProfile,
    port: u16,
    remote_sync_path: &str,
    push_source_is_dir: bool,
) -> Result<()> {
    let target_dir = remote_dir_to_create(remote_sync_path, push_source_is_dir);
    let remote_command = format!("mkdir -p {}", shell_single_quote(&target_dir));
    let status = Command::new("ssh")
        .arg("-p")
        .arg(port.to_string())
        .arg(format!("{}@{}", server.user, server.host))
        .arg(remote_command)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to start ssh for remote mkdir")?;

    if status.success() {
        Ok(())
    } else {
        bail!("Failed to create remote directory before transfer")
    }
}

fn remote_dir_to_create(remote_sync_path: &str, push_source_is_dir: bool) -> String {
    if push_source_is_dir {
        return remote_sync_path.to_string();
    }

    let trimmed = remote_sync_path.trim_end_matches('/');
    if let Some(index) = trimmed.rfind('/') {
        if index == 0 {
            "/".to_string()
        } else {
            trimmed[..index].to_string()
        }
    } else {
        ".".to_string()
    }
}

fn shell_single_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\"'\"'"))
}

fn ensure_trailing_separator(path: &str) -> String {
    if has_trailing_separator(path) {
        path.to_string()
    } else {
        format!("{}/", path)
    }
}

fn resolve_remote_sync_path(local_path: &str, remote_dir: &str) -> String {
    if !has_trailing_separator(remote_dir) {
        return remote_dir.to_string();
    }

    match path_tail_component(local_path) {
        Some(name) => {
            if remote_dir.ends_with('/') {
                format!("{}{}", remote_dir, name)
            } else {
                format!("{}/{}", remote_dir, name)
            }
        }
        None => remote_dir.to_string(),
    }
}

fn path_tail_component(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        return None;
    }
    Path::new(trimmed)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
}

fn resolve_local_gitignore_path(mode: TransferMode, local_path: &str) -> Option<PathBuf> {
    let raw = PathBuf::from(local_path);
    let start_dir = match mode {
        TransferMode::Push => {
            if raw.is_dir() {
                raw
            } else {
                raw.parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            }
        }
        TransferMode::Pull => raw,
    };

    let canonical_start = fs::canonicalize(&start_dir).unwrap_or(start_dir);
    find_local_gitignore(&canonical_start)
}

fn find_local_gitignore(start_dir: &Path) -> Option<PathBuf> {
    let mut current = Some(start_dir);
    while let Some(dir) = current {
        let candidate = dir.join(".gitignore");
        if candidate.is_file() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}
