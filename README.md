# web-agent

MCP dashboard shell. Tauri v3 (alpha) + Rust backend, CEF-capable renderer.
Pass 1 of `PLAN.md`: the dashboard, a real registry, real spawned MCP servers,
real `tools/list` results. No chats yet, no tunnel yet.

## Run

```bash
cargo tauri dev          # or: bunx @tauri-apps/cli@3.0.0-alpha.6 dev
```

`beforeDevCommand` compiles the frontend (`bun build`) and the two demo MCP
servers, so one command is enough on a clean tree.

System webview (wry) is the default runtime. To build the bundled-Chromium
renderer:

```bash
cargo tauri dev -- --features cef     # gtk4 + a ~700 MB CEF download
cargo build --no-default-features --features cef
```

`cef` and `wry` are mutually exclusive (gtk4 vs gtk3); the crate refuses to
build with both.

## Layout

```
src-tauri/src/lib.rs      app setup, state, three commands
src-tauri/src/registry.rs Anthropic JSON parsing + registry.json persistence
src-tauri/src/mcp.rs      rmcp client: spawn → handshake → tools/list
src/                      dashboard source (TypeScript, no framework)
dist/                     static shell + bun build output
tools/demo-ping           MCP server: ping, echo (read-only)
tools/demo-fs             MCP server: fs.read, scoped to the demo root
```

## Commands (Tauri IPC)

| command | purpose |
| --- | --- |
| `dashboard` | current rows + tunnel/approvals/renderer metadata |
| `refresh` | re-probe every registered server, return rows |
| `add_servers { json }` | validate Anthropic JSON, save, spawn, list tools |

Rows carry `name`, `transport`, `secret_keys`, `status`
(`pending`/`ready`/`error`), `error`, `tools`. Env **values** never leave the
backend; the UI shows key names only.

## State and secrets

- Registry: `<app data dir>/registry.json`, seeded with the two demo servers on
  first launch.
- Demo filesystem root: `<app data dir>/demo-root`.
- No shell/fs plugin is installed; the only filesystem access is `demo-fs`,
  which canonicalises every path and rejects escapes.

## Pinned versions

| crate / package | version | why |
| --- | --- | --- |
| `tauri` | `=3.0.0-alpha.6` | latest v3 alpha, has the CEF runtime + MCP plugin |
| `tauri-build` | `=3.0.0-alpha.4` | newest build crate in the v3 line |
| `tauri-runtime-wry` | `=3.0.0-alpha.6` | default runtime, matches core |
| `tauri-runtime-cef` | `=3.0.0-alpha.7` | CEF runtime; pins `cef =154.5.0` and tauri alpha.6 |
| `rmcp` | `=3.11.0` | official Rust MCP SDK (client + server) |
| `@tauri-apps/cli` (bunx) | `3.0.0-alpha.6` | only needed if `cargo tauri` is absent |

## Known gaps (next passes)

- P2: hub `POST /mcp` aggregator, OpenTunnel helper, public URL row.
- P3: per-tool Allow/Ask/Deny, restart supervision, external-kill detection.
- HTTP/SSE transports parse and persist but probe returns
  "not wired yet (P2)".
- Approvals are shown as the default policy only; nothing enforces them yet.
