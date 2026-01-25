# rup

A small interactive CLI that wraps `rsync` for fast, resumable uploads. It stores server profiles locally, lets you pick one from a menu, and then runs `rsync -avzP` with native progress output.

## Features

- Interactive menu to select or add servers
- Edit or delete existing server profiles
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

- Alias: a friendly name (e.g. "prod")
- User: SSH username
- Host: IP or domain
- Port: SSH port (defaults to 22)
- Target directory: remote destination path

Then enter a local file or folder path (you can drag it into the terminal). The tool will run:

```bash
rsync -avzP -e "ssh -p <port>" <source> <user>@<host>:<target_dir>
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
