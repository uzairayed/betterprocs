# betterprocs

Run all your project's servers and scripts in one terminal. A better alternative to [mprocs](https://github.com/pvolok/mprocs).

![Rust](https://img.shields.io/badge/built_with-Rust-orange)

## What it does

- Run multiple commands side by side in a split-pane terminal UI
- See which processes are running, stopped, or crashed at a glance
- Start, stop, and restart individual processes with a keypress
- Find and kill processes hogging your ports (built-in port killer)
- Select and copy text from process output
- Scroll through output history
- Auto-detects existing `mprocs.yaml` configs — drop-in replacement

## Install

### Homebrew (macOS)

```bash
brew install uzairayed/tap/betterprocs
```

### From source

Requires [Rust](https://rustup.rs/).

```bash
git clone https://github.com/uzairayed/betterprocs.git
cd betterprocs
cargo install --path .
```

## Usage

### Run commands directly

```bash
betterprocs "npm run dev" "npm run api" "docker compose up db"
```

### Use a config file

Create `betterprocs.yaml` in your project:

```yaml
procs:
  frontend:
    shell: npm run dev
    cwd: ./frontend
    port: 3000
  backend:
    shell: npm run server
    cwd: ./backend
    port: 8080
    env:
      NODE_ENV: development
  database:
    shell: docker compose up postgres
    autostart: false
```

Then just run:

```bash
betterprocs
```

### Load from package.json

```bash
betterprocs --npm
```

This reads all scripts from your `package.json`.

### Works with mprocs configs

If your project already has an `mprocs.yaml`, betterprocs picks it up automatically. No changes needed.

## Keyboard shortcuts

### Process list

| Key | Action |
|-----|--------|
| `j` / `k` or arrow keys | Navigate processes |
| `s` | Start process |
| `x` | Stop process |
| `X` | Force kill process |
| `r` | Restart process |
| `Tab` | Focus terminal output |
| `z` | Zoom output fullscreen |
| `` ` `` | Switch to Port Killer |
| `q` | Quit |

### Terminal output

| Key | Action |
|-----|--------|
| `Tab` | Back to process list |
| `` ` `` | Switch to Port Killer |
| `q` | Quit |
| Scroll wheel | Scroll output history |
| Click + drag | Select text (auto-copies) |

### Port Killer

| Key | Action |
|-----|--------|
| Type numbers | Filter by port (e.g. `3000,8080`) |
| Arrow keys | Navigate |
| `x` | Kill process (SIGTERM) |
| `X` | Force kill (SIGKILL) |
| `Backspace` | Delete last character |
| `Delete` | Clear filter |
| `` ` `` | Back to Processes |

### Mouse

- **Click** a process to select it
- **Click** the output pane to focus it
- **Click** `[Processes]` or `[Port Killer]` in the top bar to switch tabs
- **Drag** in the output pane to select and copy text (green flash = copied)
- **Scroll wheel** to scroll output

## Why not mprocs?

betterprocs fixes several mprocs issues:

- **Proper signal handling** — sends SIGTERM to the entire process group, not just the shell. Child processes actually get killed.
- **Port conflict detection** — on startup, detects if ports are already in use and offers to kill the conflicting processes.
- **Built-in port killer** — find and kill anything running on a port without leaving the app.
- **Better mouse support** — click to select processes, drag to copy text.
- **Running processes sort first** — active processes always appear at the top of the list.

## CLI options

```
betterprocs [OPTIONS] [COMMANDS]...

Arguments:
  [COMMANDS]...            Commands to run

Options:
  -c, --config <CONFIG>    Path to config file
      --npm                Load scripts from package.json
      --auto-exit          Quit when all processes stop
      --cwd <CWD>          Working directory
      --names <NAMES>      Process names (comma-separated)
  -h, --help               Print help
```

## License

MIT
