use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::{FilterMode, ServerProfile, SyncPreset, TransferMode, TransferPaths};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareMode {
    Quick,
    Checksum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteMode {
    KeepExtra,
    DeleteExtra,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathMode {
    ExactPath,
    DirectoryContents,
    DirectoryItself,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    LocalFile,
    LocalDirectory,
    RemotePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncPolicy {
    pub preset: SyncPreset,
    pub compare_mode: CompareMode,
    pub delete_mode: DeleteMode,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct SyncPlan {
    pub mode: TransferMode,
    pub port: u16,
    pub local_path: String,
    pub remote_input: String,
    pub resolved_remote_path: String,
    pub source_arg: String,
    pub destination_arg: String,
    pub source_kind: SourceKind,
    pub path_mode: PathMode,
    pub filter_mode: FilterMode,
    pub local_filter_file: Option<PathBuf>,
    pub policy: SyncPolicy,
    pub remote_mkdir_path: Option<String>,
}

pub fn build_sync_plan(
    server: &ServerProfile,
    mode: TransferMode,
    port: u16,
    settings: &TransferPaths,
) -> Result<SyncPlan> {
    let filter_mode = settings.effective_filter_mode();
    let policy = resolve_policy(mode, settings.sync_preset, settings.dry_run)?;

    match mode {
        TransferMode::Push => build_push_plan(server, port, settings, filter_mode, policy),
        TransferMode::Pull => build_pull_plan(server, port, settings, filter_mode, policy),
    }
}

pub fn transfer_mode_label(mode: TransferMode) -> &'static str {
    match mode {
        TransferMode::Push => "push",
        TransferMode::Pull => "pull",
    }
}

pub fn sync_preset_label(preset: SyncPreset) -> &'static str {
    match preset {
        SyncPreset::Fast => "Fast",
        SyncPreset::Strict => "Strict",
        SyncPreset::Mirror => "Mirror",
    }
}

pub fn sync_preset_description(preset: SyncPreset) -> &'static str {
    match preset {
        SyncPreset::Fast => "size + mtime comparison, keep extra files",
        SyncPreset::Strict => "checksum comparison, keep extra files",
        SyncPreset::Mirror => "checksum comparison, delete extra remote files",
    }
}

pub fn filter_mode_label(mode: FilterMode) -> &'static str {
    match mode {
        FilterMode::None => "none",
        FilterMode::LocalGitignore => "local .gitignore as rsync excludes",
        FilterMode::ExcludeGitDir => "exclude .git/",
        FilterMode::LocalGitignoreAndGitDir => "local .gitignore as rsync excludes + exclude .git/",
    }
}

pub fn compare_mode_label(mode: CompareMode) -> &'static str {
    match mode {
        CompareMode::Quick => "size + mtime",
        CompareMode::Checksum => "checksum",
    }
}

pub fn delete_mode_label(mode: DeleteMode) -> &'static str {
    match mode {
        DeleteMode::KeepExtra => "keep extra files",
        DeleteMode::DeleteExtra => "delete extra files",
    }
}

pub fn path_mode_label(mode: PathMode) -> &'static str {
    match mode {
        PathMode::ExactPath => "exact path",
        PathMode::DirectoryContents => "directory contents",
        PathMode::DirectoryItself => "directory itself",
    }
}

pub fn source_kind_label(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::LocalFile => "local file",
        SourceKind::LocalDirectory => "local directory",
        SourceKind::RemotePath => "remote path",
    }
}

pub fn preview_route(mode: TransferMode, local_path: &str, remote_path: &str) -> String {
    match mode {
        TransferMode::Push => format!("{local_path} -> {remote_path}"),
        TransferMode::Pull => format!("{remote_path} -> {local_path}"),
    }
}

fn build_push_plan(
    server: &ServerProfile,
    port: u16,
    settings: &TransferPaths,
    filter_mode: FilterMode,
    policy: SyncPolicy,
) -> Result<SyncPlan> {
    let metadata = fs::metadata(&settings.local_path)
        .with_context(|| format!("Local source not found: {}", settings.local_path))?;

    let (source_kind, path_mode, source_arg, resolved_remote_path, remote_mkdir_path) =
        if metadata.is_dir() {
            let resolved_remote_path =
                resolve_push_remote_path(&settings.local_path, &settings.remote_dir)?;
            let path_mode = if has_trailing_separator(&settings.remote_dir) {
                PathMode::DirectoryItself
            } else {
                PathMode::DirectoryContents
            };
            (
                SourceKind::LocalDirectory,
                path_mode,
                ensure_trailing_separator(&settings.local_path),
                resolved_remote_path.clone(),
                Some(resolved_remote_path),
            )
        } else if metadata.is_file() {
            let resolved_remote_path =
                resolve_push_remote_path(&settings.local_path, &settings.remote_dir)?;
            let remote_mkdir_path = remote_parent_dir(&resolved_remote_path);
            (
                SourceKind::LocalFile,
                PathMode::ExactPath,
                settings.local_path.clone(),
                resolved_remote_path,
                Some(remote_mkdir_path),
            )
        } else {
            bail!("Local source must be a regular file or directory")
        };

    if matches!(policy.delete_mode, DeleteMode::DeleteExtra)
        && !matches!(source_kind, SourceKind::LocalDirectory)
    {
        bail!("Mirror strategy requires a directory source in push mode")
    }

    let local_filter_file = resolve_local_filter_file(filter_mode, TransferMode::Push, &settings.local_path)?;
    let destination_arg = format!(
        "{}@{}:{}",
        server.user, server.host, resolved_remote_path
    );

    Ok(SyncPlan {
        mode: TransferMode::Push,
        port,
        local_path: settings.local_path.clone(),
        remote_input: settings.remote_dir.clone(),
        resolved_remote_path,
        source_arg,
        destination_arg,
        source_kind,
        path_mode,
        filter_mode,
        local_filter_file,
        policy,
        remote_mkdir_path,
    })
}

fn build_pull_plan(
    server: &ServerProfile,
    port: u16,
    settings: &TransferPaths,
    filter_mode: FilterMode,
    policy: SyncPolicy,
) -> Result<SyncPlan> {
    if matches!(policy.delete_mode, DeleteMode::DeleteExtra) {
        bail!("Mirror strategy is only supported in push mode")
    }

    let local_destination = Path::new(&settings.local_path);
    if local_destination.exists() && !local_destination.is_dir() {
        bail!("Local destination must be a directory for pull mode")
    }

    let resolved_remote_path = if has_trailing_separator(&settings.remote_dir) {
        ensure_trailing_separator(&settings.remote_dir)
    } else {
        settings.remote_dir.clone()
    };
    let source_arg = format!("{}@{}:{}", server.user, server.host, resolved_remote_path);
    let local_filter_file =
        resolve_local_filter_file(filter_mode, TransferMode::Pull, &settings.local_path)?;
    let path_mode = if has_trailing_separator(&settings.remote_dir) {
        PathMode::DirectoryContents
    } else {
        PathMode::ExactPath
    };

    Ok(SyncPlan {
        mode: TransferMode::Pull,
        port,
        local_path: settings.local_path.clone(),
        remote_input: settings.remote_dir.clone(),
        resolved_remote_path,
        source_arg,
        destination_arg: settings.local_path.clone(),
        source_kind: SourceKind::RemotePath,
        path_mode,
        filter_mode,
        local_filter_file,
        policy,
        remote_mkdir_path: None,
    })
}

fn resolve_policy(mode: TransferMode, preset: SyncPreset, dry_run: bool) -> Result<SyncPolicy> {
    if matches!(mode, TransferMode::Pull) && matches!(preset, SyncPreset::Mirror) {
        bail!("Mirror strategy is only supported in push mode")
    }

    let (compare_mode, delete_mode) = match preset {
        SyncPreset::Fast => (CompareMode::Quick, DeleteMode::KeepExtra),
        SyncPreset::Strict => (CompareMode::Checksum, DeleteMode::KeepExtra),
        SyncPreset::Mirror => (CompareMode::Checksum, DeleteMode::DeleteExtra),
    };

    Ok(SyncPolicy {
        preset,
        compare_mode,
        delete_mode,
        dry_run,
    })
}

fn has_trailing_separator(path: &str) -> bool {
    path.ends_with('/') || path.ends_with('\\')
}

fn ensure_trailing_separator(path: &str) -> String {
    if has_trailing_separator(path) {
        path.to_string()
    } else {
        format!("{path}/")
    }
}

fn resolve_push_remote_path(local_path: &str, remote_input: &str) -> Result<String> {
    if !has_trailing_separator(remote_input) {
        return Ok(remote_input.to_string());
    }

    let tail = path_tail_component(local_path)
        .ok_or_else(|| anyhow::anyhow!("Failed to resolve local path tail component"))?;
    if remote_input.ends_with('/') {
        Ok(format!("{remote_input}{tail}"))
    } else {
        Ok(format!("{remote_input}/{tail}"))
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

fn remote_parent_dir(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
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

fn resolve_local_filter_file(
    filter_mode: FilterMode,
    mode: TransferMode,
    local_path: &str,
) -> Result<Option<PathBuf>> {
    if !matches!(
        filter_mode,
        FilterMode::LocalGitignore | FilterMode::LocalGitignoreAndGitDir
    ) {
        return Ok(None);
    }

    resolve_local_gitignore_path(mode, local_path)
        .map(Some)
        .ok_or_else(|| anyhow::anyhow!("Local .gitignore not found for the selected local path"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        ServerProfile, SyncPreset, TransferPaths, default_filter_mode, default_ssh_port,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("sync-helper-{name}-{unique}"))
    }

    fn test_server() -> ServerProfile {
        ServerProfile {
            user: "alice".to_string(),
            host: "example.com".to_string(),
            port: default_ssh_port(),
            push_defaults: None,
            pull_defaults: None,
            shared_paths: None,
            push_paths: None,
            pull_paths: None,
        }
    }

    #[test]
    fn push_dir_with_base_dir_preserves_directory_name() {
        let source = temp_path("planner-dir");
        fs::create_dir_all(&source).expect("create dir");
        let tail = source
            .file_name()
            .expect("dir name")
            .to_string_lossy()
            .to_string();

        let settings = TransferPaths {
            local_path: source.to_string_lossy().to_string(),
            remote_dir: "/srv/releases/".to_string(),
            sync_preset: SyncPreset::Strict,
            filter_mode: Some(default_filter_mode()),
            dry_run: false,
            use_gitignore: false,
            ignore_git_dir: true,
        };

        let plan = build_sync_plan(&test_server(), TransferMode::Push, 22, &settings)
            .expect("push plan");

        assert_eq!(plan.path_mode, PathMode::DirectoryItself);
        assert_eq!(plan.resolved_remote_path, format!("/srv/releases/{tail}"));
        assert!(plan.source_arg.ends_with('/'));

        fs::remove_dir_all(source).expect("cleanup");
    }

    #[test]
    fn mirror_requires_push_directory() {
        let source = temp_path("planner-file.txt");
        fs::write(&source, "hello").expect("write file");

        let settings = TransferPaths {
            local_path: source.to_string_lossy().to_string(),
            remote_dir: "/srv/app.txt".to_string(),
            sync_preset: SyncPreset::Mirror,
            filter_mode: Some(default_filter_mode()),
            dry_run: false,
            use_gitignore: false,
            ignore_git_dir: true,
        };

        let error = build_sync_plan(&test_server(), TransferMode::Push, 22, &settings)
            .expect_err("mirror should fail");

        assert!(error.to_string().contains("directory source"));
        fs::remove_file(source).expect("cleanup");
    }

    #[test]
    fn pull_cannot_use_mirror_preset() {
        let settings = TransferPaths {
            local_path: temp_path("pull-destination").to_string_lossy().to_string(),
            remote_dir: "/srv/app/".to_string(),
            sync_preset: SyncPreset::Mirror,
            filter_mode: Some(default_filter_mode()),
            dry_run: false,
            use_gitignore: false,
            ignore_git_dir: true,
        };

        let error = build_sync_plan(&test_server(), TransferMode::Pull, 22, &settings)
            .expect_err("mirror should fail");

        assert!(error.to_string().contains("push mode"));
    }
}
