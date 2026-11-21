# bionichat

MCP dashboard shell. Tauri v3 (alpha) + Rust backend, CEF-capable renderer.
Passes 1-3 of `PLAN.md`: the dashboard, a real registry, real spawned MCP
servers, a local hub that aggregates them behind one MCP endpoint, the
OpenTunnel button, per-tool approval policy, and supervision. No chats yet.

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

CEF needs Chromium's SUID sandbox helper to be root-owned, otherwise the
renderer aborts at startup. The build already installs a correct helper under
`target/cef/debug/`, so run the app through the wrapper — it hides the
non-setuid copy the build also drops beside the binary and points
`CHROME_DEVEL_SANDBOX` at the real one, no sudo needed:

```bash
scripts/run-cef.sh            # same binary, CEF sandbox satisfied
```

If you prefer the manual route instead:

```bash
sudo chown root:root target/debug/chrome-sandbox
sudo chmod 4755 target/debug/chrome-sandbox
```

Verified 2026-11-21: the CEF build renders the full P3 dashboard (policy
dropdowns, restart button, auto-restart toggle, approvals banner); build takes
~4 min and an 8.1 GB `target/cef` (downloads `cef 154.5.0`). The renderer chip
in the header shows which runtime built ("cef" or "wry").


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
| `dashboard` | rows + hub + tunnel + approvals/renderer metadata |
| `refresh` | re-probe every registered server through the hub sessions |
| `add_servers { json }` | validate Anthropic JSON, save, spawn, list tools |
| `tunnel_start` | spawn `opentunnel route add <port>`, scrape URL, poll it |
| `tunnel_stop` | kill the tunnel child, back to `off` |

## Hub

`http://127.0.0.1:8787/mcp` (override with `BIONICHAT_HUB_PORT`). JSON-RPC over
POST, no SSE. Tools are namespaced `server__tool`, so `demo-ping__ping` routes
to `ping` on the `demo-ping` server over that server's live stdio session.

```bash
curl -s -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' \
  http://127.0.0.1:8787/mcp

curl -s -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call",
       "params":{"name":"demo-ping__ping","arguments":{}}}' \
  http://127.0.0.1:8787/mcp
```

Handled: `initialize`, `notifications/*` (202), `ping`, `tools/list`,
`tools/call`; anything else returns `-32601`, unknown tool names `-32602`,
malformed JSON HTTP 400, `GET /mcp` 405. A server that fails to start is left
out of the union and reported in the dashboard row, not in the hub response.

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

## Tunnel

One click runs `opentunnel route add <port>` (or `$OPENTUNNEL_BIN`), scrapes the
`https://…` line it prints, then polls `<url>/mcp` with an MCP `ping` for up to
60 s before showing `ready`.

- **Install** (user-local, no sudo, no account): `npm install -g --prefix "$HOME/.local" opentunnel`
  → `~/.local/bin/opentunnel` (6.5 MB). Removal: `npm uninstall -g --prefix "$HOME/.local" opentunnel`
  and `rm -rf ~/.local/state/opentunnel`.
- **Side effect**: `route add` registers and enables a systemd *user* service
  (`opentunnel-default.service`) so the tunnel survives reboots. `Stop` runs
  `opentunnel down`, which stops it and removes that unit. Stop is per-profile,
  not per-app: it also takes down a tunnel started outside this app.
- Routes stay configured while stopped, so starting again reuses the same URL.
- Public URL = public. It serves the hub as-is: demo data only, no auth.

Verified 2026-11-20 on this box (tunnel `kfgepc72im6k.opentunnel.xyz`):

| check | result |
| --- | --- |
| app `Start tunnel` | `ready`, URL shown in the card |
| TLS chain | `ZeroSSL ECC DV SSL CA 2`, openssl verify `0 (ok)` |
| `POST /mcp tools/list` from the internet | `demo-ping__ping`, `demo-ping__echo`, `demo-fs__fs.read` |
| `POST /mcp tools/call demo-ping__ping` | `pong` |
| app `Stop` | service stopped, public URL stops answering |

## Known gaps (next passes)

- P3: per-tool Allow/Ask/Deny, restart supervision, external-kill detection,
  re-reading the registry when the file changes outside the app.
- Hub is JSON-RPC over POST only; SSE streaming and `Mcp-Session-Id` are not
  implemented, so clients that require SSE will not connect.
- Tool calls through the hub are unbounded: no per-call timeout, and the
  approvals policy is not enforced yet.
- HTTP/SSE transports parse and persist but probe returns
  "not wired yet (P2)".
- Approvals are shown as the default policy only; nothing enforces them yet.
