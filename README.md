# youtrack-mcp-rs

An MCP (Model Context Protocol) server for [YouTrack](https://www.jetbrains.com/youtrack/) —
issue search and CRUD, agile boards, the knowledge base, workflow commands,
comments, attachments, users, time tracking, and issue history — for AI
coding agents that speak MCP over stdio.

> This project is not affiliated with or endorsed by JetBrains. YouTrack is a trademark of JetBrains s.r.o.

## Status

All 34 tools call the real YouTrack REST API. The full test suite (234
tests, including a stdio-transport end-to-end test) passes. This crate has
not yet been published to crates.io or tagged with a GitHub release — see
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
tagged yet. Until then, build from source:

```bash
git clone https://github.com/Guillermo-Octavio/youtrack-mcp-rs
cd youtrack-mcp-rs
cargo build --release
# binary at target/release/youtrack-mcp
```

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

Add the server as a stdio-based MCP server. This example uses the
`claude_desktop_config.json` shape; any MCP client that launches stdio
servers accepts an equivalent entry:

```json
{
  "mcpServers": {
    "youtrack": {
      "command": "/path/to/youtrack-mcp",
      "env": {
        "YOUTRACK_URL": "https://example.myjetbrains.com",
        "YOUTRACK_TOKEN": "perm:xxxxxxxxxxxxxxxxxxxxxxxx"
      }
    }
  }
}
```

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
