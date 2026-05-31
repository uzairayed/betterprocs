# Changelog

All notable changes to betterprocs are documented here. This project adheres to
[Semantic Versioning](https://semver.org/).

## [0.3.0] - 2026-05-31

### Added
- **MCP server mode** (`betterprocs mcp`) — a headless [Model Context
  Protocol](https://modelcontextprotocol.io) server over stdio that lets an AI
  agent launch, read, drive, and stop processes without the TUI.
  - 13 tools: `list_processes`, `run_command`, `start_process`, `stop_process`,
    `restart_process`, `force_kill_process`, `read_output`, `copy_output`,
    `send_input`, `find_port`, `kill_port`, `search_processes`, `kill_pid`.
  - Config sources (`--config`, `--npm`) work with the subcommand; the agent can
    also start with no processes and launch everything via `run_command`.

## [0.2.0] - 2026

### Added
- Clear logs for the selected process with the `c` key.

### Changed
- Install docs: Linux Homebrew support and `cargo install` instructions.

## [0.1.0]

### Added
- Initial release: split-pane terminal UI for running multiple processes.
- Start, stop, restart, and force-kill processes; running processes sort first.
- Process-group signal handling (SIGTERM/SIGKILL to the whole tree).
- Port conflict detection on startup and a built-in port killer.
- Mouse support, output scrolling, and text selection/copy.
- Config from CLI args, YAML (`betterprocs.yaml` / `mprocs.yaml`), or
  `package.json` scripts.

[0.3.0]: https://github.com/uzairayed/betterprocs/releases/tag/v0.3.0
[0.2.0]: https://github.com/uzairayed/betterprocs/releases/tag/v0.2.0
[0.1.0]: https://github.com/uzairayed/betterprocs/releases/tag/v0.1.0
