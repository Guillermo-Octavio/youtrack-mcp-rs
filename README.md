# youtrack-mcp-rs

An MCP (Model Context Protocol) server for [YouTrack](https://www.jetbrains.com/youtrack/) —
issue search and CRUD, agile boards, the knowledge base, workflow commands,
comments, attachments, users, time tracking, and issue history — for AI
coding agents that speak MCP over stdio.

> This project is not affiliated with or endorsed by JetBrains. YouTrack is a trademark of JetBrains s.r.o.

## Status

All 34 tools call the real YouTrack REST API. The full test suite (234
tests, including a stdio-transport end-to-end test) passes — see
[Install](#install) below.

## Quick path

1. Build the server (see [Install](#install)).
2. Set `YOUTRACK_URL` and `YOUTRACK_TOKEN` (see [Configuration](#configuration)).
3. Point your MCP client at the `youtrack-mcp` binary over stdio (see
   [MCP client configuration](#mcp-client-configuration)).
4. Confirm it starts: the server validates connectivity against YouTrack
   before it serves any tool calls, and exits with an actionable error if
   the URL or token is wrong.

## Install

This crate is not yet published to crates.io and no GitHub release has been
tagged yet. Until then, build from source. The steps below cover Linux,
macOS, Windows 11 (native), and WSL2.

### 1. Install a Rust toolchain

- **Linux / macOS / WSL2**

  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source "$HOME/.cargo/env"
  ```

- **Windows 11 (native)**

  Rust needs the MSVC linker, so install the Visual Studio Build Tools first,
  then `rustup`:

  ```powershell
  winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools"
  winget install Rustlang.Rustup
  ```

  (Alternatively, download `rustup-init.exe` from <https://rustup.rs>.)

Verify with `rustc --version` in a new shell.

### 2. Build the binary

Same command on every platform:

```bash
git clone https://github.com/Guillermo-Octavio/youtrack-mcp-rs
cd youtrack-mcp-rs
cargo build --release
```

The binary is written to:

| Platform | Binary path |
|---|---|
| Linux / macOS / WSL2 | `target/release/youtrack-mcp` |
| Windows 11 (native) | `target\release\youtrack-mcp.exe` |

Two distribution paths are planned once the project is tagged and published:

- **`cargo install youtrack-mcp-rs`** — once the crate is published to
  crates.io.
- **Prebuilt binary download** — once a `v*` tag is pushed, the
  [release workflow](.github/workflows/release.yml) builds and attaches a
  Linux x86_64 binary to the corresponding GitHub Release; download it from
  the repository's Releases page.

## Configuration

The server reads two environment variables at startup:

| Variable | Required | Description |
|---|---|---|
| `YOUTRACK_URL` | Yes | Absolute `https://` URL of your YouTrack instance, without a trailing `/api` suffix (e.g. `https://example.myjetbrains.com`). |
| `YOUTRACK_TOKEN` | Yes | A YouTrack permanent token with access to the projects, articles, and boards you want this server to reach. |

On every startup the server probes `YOUTRACK_URL` with `YOUTRACK_TOKEN`
before serving any MCP request, and fails fast with an actionable error
message if the URL is misconfigured or the token is invalid or lacks
permission — it never starts serving tools against a connection it hasn't
verified.

## MCP client configuration

The server speaks MCP over stdio, so any client that can launch a local
process works. Below are step-by-step instructions for [Claude Code](#claude-code)
and [OpenCode](#opencode). In every example, replace `/path/to/youtrack-mcp`
with the absolute path to the binary you built above.

### Claude Code

If the `claude` CLI isn't installed yet:

| Platform | Install command |
|---|---|
| Linux / macOS / WSL2 | `curl -fsSL https://claude.ai/install.sh \| bash` |
| Windows 11 (PowerShell) | `irm https://claude.ai/install.ps1 \| iex` |
| Windows 11 (WinGet) | `winget install Anthropic.ClaudeCode` |
| Any platform (npm, needs Node.js 22+) | `npm install -g @anthropic-ai/claude-code` |

Verify with `claude --version`.

**Register the server** with the CLI:

```bash
claude mcp add --scope user \
  --env YOUTRACK_URL=https://example.myjetbrains.com \
  --env YOUTRACK_TOKEN=perm:xxxxxxxxxxxxxxxxxxxxxxxx \
  youtrack -- /path/to/youtrack-mcp
```

`--scope` controls where the entry is written: `local` (current project
only, the default), `project` (writes `.mcp.json`, shared with the team via
git), or `user` (writes `~/.claude.json`, available in every project).

Equivalently, edit `.mcp.json` (or the user-scoped config) by hand:

```json
{
  "mcpServers": {
    "youtrack": {
      "type": "stdio",
      "command": "/path/to/youtrack-mcp",
      "args": [],
      "env": {
        "YOUTRACK_URL": "https://example.myjetbrains.com",
        "YOUTRACK_TOKEN": "perm:xxxxxxxxxxxxxxxxxxxxxxxx"
      }
    }
  }
}
```

**Verify it's registered:**

```bash
claude mcp list
claude mcp get youtrack
```

or run `/mcp` inside a Claude Code session.

**Platform notes:**

- Windows 11 (native): use the `.exe` path, e.g.
  `"command": "C:\\path\\to\\youtrack-mcp.exe"` (escape backslashes in JSON,
  or use forward slashes).
- WSL2: keep the Claude Code session and the binary on the same side. If
  Claude Code runs inside WSL2, point `command` at the Linux binary
  (`target/release/youtrack-mcp`, or `/mnt/c/...` if it lives on the Windows
  filesystem) — don't mix a WSL2 session with a native-Windows-built `.exe`.

### OpenCode

If OpenCode isn't installed yet:

| Platform | Install command |
|---|---|
| Linux / macOS / WSL2 | `curl -fsSL https://opencode.ai/install \| bash` |
| Windows 11 (npm, needs Node.js) | `npm install -g opencode-ai` |
| Windows 11 (Scoop) | `scoop install opencode` |
| macOS (Homebrew) | `brew install sst/tap/opencode` |

WSL2 is the recommended way to run OpenCode on Windows for full
compatibility; native Windows support is secondary but works.

**Register the server** in `opencode.json` — either project-scoped (repo
root) or global (`~/.config/opencode/opencode.json` on Linux/macOS/WSL2):

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "youtrack": {
      "type": "local",
      "command": ["/path/to/youtrack-mcp"],
      "enabled": true,
      "environment": {
        "YOUTRACK_URL": "https://example.myjetbrains.com",
        "YOUTRACK_TOKEN": "perm:xxxxxxxxxxxxxxxxxxxxxxxx"
      }
    }
  }
}
```

Note the differences from Claude Code's schema: the top-level key is `mcp`
(not `mcpServers`), `command` is an array (not a string), and the
environment key is `environment` (not `env`).

**Verify it's registered:**

```bash
opencode mcp list
opencode mcp debug youtrack
```

**Platform notes:**

- Windows 11 (native): use the `.exe` path in the `command` array, e.g.
  `"command": ["C:\\path\\to\\youtrack-mcp.exe"]`.
- WSL2: same rule as Claude Code — if OpenCode runs inside WSL2, point
  `command` at the Linux binary, not a native-Windows `.exe`.

## Tools

34 tools, grouped by domain. This list must stay in sync with
`EXPECTED_TOOL_NAMES` in [`tests/e2e_stdio.rs`](tests/e2e_stdio.rs), which
asserts the running server exposes exactly these names — if you add,
rename, or remove a tool, update both places together.

| Domain | Tools |
|---|---|
| Search & Read | `search_issues`, `get_issue`, `list_projects`, `list_project_fields`, `list_links` |
| Agile | `list_agile_boards`, `list_sprints`, `get_sprint` |
| Issue CRUD | `create_issue`, `update_issue`, `delete_issue` |
| Knowledge Base | `list_articles`, `get_article`, `create_article`, `update_article`, `delete_article`, `list_article_attachments`, `upload_article_attachment` |
| Workflow Commands | `apply_command`, `bulk_command` |
| Comments | `list_comments`, `add_comment` |
| Attachments | `list_attachments`, `upload_attachment`, `download_attachment` |
| Users | `get_current_user`, `search_users`, `list_watchers` |
| Time Tracking | `list_work_items`, `add_work_item`, `delete_work_item` |
| History | `get_issue_activity` |
| Misc | `list_tags`, `list_saved_searches` |

Every tool returns both a human-readable text rendering and a structured
JSON payload (MCP `structuredContent`) for the same call, so chat-style and
structured-data MCP clients can both consume the result.

## Building

```bash
cargo build
cargo test
```

## Contributing

Issues and pull requests are welcome. Before opening a PR:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

These are the same checks [CI](.github/workflows/ci.yml) runs on every
push and pull request against `main`.

## License

MIT — see [LICENSE](LICENSE).
