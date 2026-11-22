# bionichat

MCP dashboard shell. Tauri v3 (alpha) + Rust backend, CEF-capable renderer.
Passes 1-4 of `PLAN.md`: the dashboard, a real registry, real file and shell
MCP servers (`fs` with read/write/edit, `bash` with one shell tool), a
local hub that aggregates them behind one MCP endpoint and forwards real
input schemas, the OpenTunnel button, per-tool approval policy, and
supervision. Every tool runs without asking: a web chat gives up in seconds,
so an approval prompt reads as a broken bridge. Set a tool to Ask or Deny in
Settings when you want a brake, and Revoke link is the real kill switch. Every
result carries the server clock inside its own text block, because some
clients render only one block.

The chat runs in the app: pick a project folder, talk to any
OpenAI-compatible model, and its tool calls go back through the same hub, so
policies, activity and the clock behave exactly as they do for a web client.
The model key lives in `model.json` (0600) and never reaches the page.

## The public link

A chat app on the web cannot reach `127.0.0.1`, so the app puts a tunnel in
front of the hub. Two helpers, chosen automatically:

| helper | URL | where it comes from |
|---|---|---|
| `opentunnel` | stable, one per machine | used when you already have it (Linux/macOS) |
| `cloudflared` | fresh on every start | **ships with the app**, nothing to install |

`bun run build` downloads the cloudflared binary for your platform and puts it
beside the app binary, so a copy of the build folder needs nothing installed.
OpenTunnel wins when present because its URL survives restarts; cloudflared is
the zero-setup fallback. Set `BIONICHAT_TUNNEL=cloudflared` to force the
bundled one.

Stop kills the tunnel and keeps the link reusable (OpenTunnel). Revoke burns
it: OpenTunnel deletes the route, cloudflared simply dies and the next start
mints a new URL. Either way a leaked link cannot be reused after Revoke.

## Platforms

Linux is the development target and the only place this has been run. Windows
is supported in the code (see `WINDOWS.md` for what changed, what is unproven
and how to build it). macOS has not been touched.

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
src-tauri/src/lib.rs      app setup, state, commands
src-tauri/src/registry.rs Anthropic JSON parsing + registry.json persistence
src-tauri/src/mcp.rs      rmcp client: spawn → handshake → tools/list → call
src-tauri/src/hub.rs      one MCP endpoint over every server, policy, activity
src-tauri/src/tunnel.rs   opentunnel route lifecycle
src/                      dashboard source (React + Vite + Tailwind)
  views/                  Overview, Projects, Settings
  components/ToolFlow.tsx live graph of hub → servers → tools
  components/LiveRail.tsx running and recent calls
tools/fs                  MCP server: read, write, edit
tools/bash                MCP server: bash
tools/skill               MCP server: skill (list + read from .agents/skills)
dist/                     bun run build output
```

## Tool names

Every tool is namespaced by its server, so a name never lies about where it
came from and two servers can never collide:

- built-ins share the `core` namespace: `core_read`, `core_write`,
  `core_edit`, `core_bash`, `core_skill`
- `core_read` answers images (PNG, JPEG, GIF, WebP, BMP, up to 8 MB) as an MCP
  image block next to a text line, so a vision-capable chat can look at a
  screenshot or a photo instead of being told it is binary
- every other server uses its own name: `context7_query-docs`

A call needs no `tools/list` first: the hub resolves the prefix itself, so the
first call after a restart is as fast as the tenth. Listings answer from the
last known listing, so a slow server (anything behind `npx`) cannot stall an
agent's first call.

Servers are added and removed from Settings → Servers. Removing one drops its
process, its tools and its stored policies. The built-in `core` servers cannot
be removed: they are the app's own hands, and the backend refuses even if a
call comes from somewhere else. A registry file that will not
parse is never overwritten: the app keeps a `.bad-<epoch>` copy and refuses to
reseed over it.

## The live map

Settings → Tools draws the hub, every server and every tool as a graph, and
lights the path up while a call is in flight.

Projects is a picker, not a listing: you choose any folder on this machine and
it becomes a project the chat will work in. Saved in
`<data>/projects.json`; a folder that later disappears drops itself. The public link itself is never printed there, only
inside Settings → Tunnel, where copying happens. Projects lists the folders in `~/Projects` (your own
work, not MCP servers). Everything about the bridge — servers, tool policies,
the live map, the tunnel — lives in Settings, pinned to the bottom of the
sidebar.

- The hub emits `tool-start` the moment a call arrives, before policy runs,
  and `tool-activity` when it finishes. The graph marks the tool blue with a
  dashed edge from the hub while it runs.
- A finished call keeps a ring for 6 seconds (green ok, red failure), then
  fades. Nothing stays coloured forever.
- Followed calls read like a terminal, not JSON: the rail shows the kind
  (Shell / Read / Write / Edit), the command or path, then the output, with
  a Show all toggle for long runs.
- Navigation is custom, so it works in the embedded CEF window: pan and zoom
  the canvas, click a tool for its description.
- `bun run debug-page` writes `dist/_debug.html`: the real bundle against
  fixture data, so the UI can be driven in a normal browser with DevTools.


## Commands (Tauri IPC)

| command | purpose |
| --- | --- |
| `dashboard` | rows + hub + tunnel + approvals/renderer metadata |
| `refresh` | re-probe every registered server through the hub sessions |
| `add_servers { json }` | validate Anthropic JSON, save, spawn, list tools |
| `tunnel_start` | spawn `opentunnel route add <port>`, scrape URL, poll it |
| `tunnel_stop` | kill the tunnel child, back to `off` |
| `tunnel_revoke` | remove the route (URL dies), then stop |
| `resolve_session` | approve once + remember until the app quits |

## Hub

`http://127.0.0.1:8787/mcp` (override with `BIONICHAT_HUB_PORT`). JSON-RPC over
POST, no SSE. Every tool result carries a server-time stamp (`[server time <local> = <UTC>]`) so web agents stay time-aware.

Tools keep their plain names while unique (`read`, `bash`); on a collision they fall back to `server__tool`.
Calls route to the owning server over its live stdio session.

```bash
curl -s -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' \
  http://127.0.0.1:8787/mcp

curl -s -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call",
       "params":{"name":"read","arguments":{"path":"demo.txt"}}}' \
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
- **Revoke link** burns the route (`opentunnel route remove`) and stops the
  service: the URL dies for good, the next start mints a fresh random one.
  Use it when a link leaks.
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
