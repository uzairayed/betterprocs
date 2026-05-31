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

### Homebrew (macOS and Linux)

```bash
brew install uzairayed/tap/betterprocs
```

Works with [Homebrew on Linux](https://docs.brew.sh/Homebrew-on-Linux) too.

### Cargo

Requires [Rust](https://rustup.rs/).

```bash
cargo install betterprocs
```

### From source

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

## MCP server (AI agents)

betterprocs can run headless as an [MCP](https://modelcontextprotocol.io) server over stdio,
letting an AI agent launch, read, drive, and stop processes for you — no TUI:

```bash
betterprocs mcp                       # agent launches everything via run_command
betterprocs mcp --config procs.yaml   # preload processes from a config
betterprocs --npm mcp                 # preload package.json scripts
```

Add it to an MCP client (e.g. Claude Code / Claude Desktop):

```json
{
  "mcpServers": {
    "betterprocs": { "command": "betterprocs", "args": ["mcp"] }
  }
}
```

Tools exposed:

| Tool | What it does |
|------|--------------|
| `list_processes` | Status, pid, command, cwd, and port for every managed process |
| `run_command` | Launch a new arbitrary shell command as a managed process |
| `start_process` / `stop_process` / `restart_process` / `force_kill_process` | Control a process by name |
| `read_output` | Read recent terminal output (including scrollback) |
| `copy_output` | Same as `read_output`, and copies the text to the OS clipboard |
| `send_input` | Type into a process's stdin |
| `find_port` / `kill_port` | Find or kill whatever is listening on a TCP port |
| `search_processes` / `kill_pid` | Search all OS processes by name/command, or kill one by PID |

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
betterprocs [OPTIONS] [COMMANDS]... [COMMAND]

Commands:
  mcp                      Run as a headless MCP server over stdio (for AI agents)

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
