# rup

A small interactive CLI that wraps `rsync` for fast, resumable uploads. It stores minimal server profiles locally (user + host), lets you pick one from a menu, and then runs `rsync -avzP` with native progress output.

## Features

- Interactive menu to select or add servers
- Edit or delete existing server profiles
- Supports push (local → remote) and pull (local ← remote) modes per run
- Simple config stored in the OS config directory
- Path sanitization for drag-and-drop (strips quotes)
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

The app only saves the user and host. Port, transfer mode, and paths are chosen each run.

Then select the transfer mode, enter the port, and provide both paths. The tool will run:

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

## License

MIT
