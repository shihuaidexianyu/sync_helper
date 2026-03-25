use anyhow::{Context, Result, bail};
use console::style;
use std::process::{Command, Stdio};

use crate::models::{FilterMode, ServerProfile};
use crate::planner::{CompareMode, DeleteMode, SyncPlan};

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

pub fn run_sync_plan(server: &ServerProfile, plan: &SyncPlan) -> Result<()> {
    if let Some(path) = &plan.remote_mkdir_path {
        ensure_remote_path(server, plan.port, path)?;
    }

    let args = build_rsync_args(plan);
    let status = Command::new("rsync")
        .args(&args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to start rsync")?;

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

pub fn build_rsync_args(plan: &SyncPlan) -> Vec<String> {
    let mut args = vec![
        "-azP".to_string(),
        "--itemize-changes".to_string(),
        "--stats".to_string(),
        "-e".to_string(),
        format!("ssh -p {}", plan.port),
    ];

    if matches!(plan.policy.compare_mode, CompareMode::Checksum) {
        args.push("--checksum".to_string());
    }
    if matches!(plan.policy.delete_mode, DeleteMode::DeleteExtra) {
        args.push("--delete-delay".to_string());
    }
    if plan.policy.dry_run {
        args.push("--dry-run".to_string());
    }
    if let Some(path) = &plan.local_filter_file {
        args.push("--exclude-from".to_string());
        args.push(path.display().to_string());
    }
    if matches!(
        plan.filter_mode,
        FilterMode::ExcludeGitDir | FilterMode::LocalGitignoreAndGitDir
    ) {
        args.push("--exclude=.git/".to_string());
    }

    args.push(plan.source_arg.clone());
    args.push(plan.destination_arg.clone());
    args
}

fn ensure_remote_path(server: &ServerProfile, port: u16, path: &str) -> Result<()> {
    let remote_command = format!("mkdir -p {}", shell_single_quote(path));
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

fn shell_single_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FilterMode, SyncPreset, TransferMode};
    use crate::planner::{CompareMode, DeleteMode, PathMode, SourceKind, SyncPlan, SyncPolicy};

    fn sample_plan() -> SyncPlan {
        SyncPlan {
            mode: TransferMode::Push,
            port: 2222,
            local_path: "/tmp/src".to_string(),
            remote_input: "/srv/app".to_string(),
            resolved_remote_path: "/srv/app".to_string(),
            source_arg: "/tmp/src/".to_string(),
            destination_arg: "alice@example.com:/srv/app".to_string(),
            source_kind: SourceKind::LocalDirectory,
            path_mode: PathMode::DirectoryContents,
            filter_mode: FilterMode::ExcludeGitDir,
            local_filter_file: None,
            policy: SyncPolicy {
                preset: SyncPreset::Mirror,
                compare_mode: CompareMode::Checksum,
                delete_mode: DeleteMode::DeleteExtra,
                dry_run: true,
            },
            remote_mkdir_path: Some("/srv/app".to_string()),
        }
    }

    #[test]
    fn mirror_plan_builds_strict_args() {
        let args = build_rsync_args(&sample_plan());

        assert!(args.iter().any(|arg| arg == "--checksum"));
        assert!(args.iter().any(|arg| arg == "--delete-delay"));
        assert!(args.iter().any(|arg| arg == "--dry-run"));
        assert!(args.iter().any(|arg| arg == "--exclude=.git/"));
    }
}
