# rup

A small interactive CLI that wraps `rsync` for fast, resumable uploads. It stores lightweight server profiles locally, lets you pick one from a menu, and then runs `rsync -avzP` with native progress output.

## Features

- Interactive menu to select or add servers
- Manage (edit/delete) server profiles from a dedicated menu
- Supports push (local → remote) and pull (local ← remote) modes per run
- Optional local `.gitignore` filtering for both push and pull
- Optional exclusion of `.git/` during transfer
- Simple config stored in the OS config directory
- Path sanitization for drag-and-drop (strips quotes)
- Remembers one shared set of port/paths/filters per server (used by both push and pull)
- Uses `rsync` directly with inherited output for native progress

## Requirements

- Rust (stable)
- `rsync` available in `PATH`
- SSH access to your servers (key-based auth recommended)

## Install

```bash
cargo build --release
```

Binary will be at `target/release/rup`.

## Usage

```bash
cargo run
```

On first run, you'll be prompted to add a server profile:

- User: SSH username
- Host: IP or domain

The app stores user/host, and remembers one shared transfer setup per server.

Then select a server and go directly to transfer mode. The app reuses the same saved port/paths/filters for both push and pull, and applies them as defaults next time.
If the remote path ends with `/`, it is treated as a base directory and the app appends the local folder name automatically (for same-name sync in both directions).

If the selected mode has history, you can directly reuse the last settings in one step.

You can also choose whether to apply local `.gitignore` rules during a run (works for both push and pull).
Filtering is selected from a single menu: none / `.gitignore` / exclude `.git/` / both.

Before execution, the app prints a transfer summary and asks for final confirmation.

The tool will run:

```bash
# push mode
rsync -avzP -e "ssh -p <port>" <local_source> <user>@<host>:<remote_dir>

# pull mode
rsync -avzP -e "ssh -p <port>" <user>@<host>:<remote_dir> <local_destination>
```

## Config location

The config file is stored at the OS standard config directory for this app:

- Windows: `%APPDATA%\rup\config.toml`
- macOS: `~/Library/Application Support/rup/config.toml`
- Linux: `~/.config/rup/config.toml`

## Notes

- If `config.toml` is corrupted, the app will ask whether to reset it.
- Transfer errors are shown directly from `rsync`.
- `.gitignore` filtering reads the local `.gitignore` file and passes it to `rsync --exclude-from` for both push and pull.
- `.git/` exclusion uses `rsync --exclude=.git/` when enabled.
- In push mode, the app runs `ssh "mkdir -p ..."` before `rsync` to create the remote target path (or parent path for single-file uploads).

## License

MIT
