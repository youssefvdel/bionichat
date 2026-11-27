# Handoff — 2026-11-27

Written for a fresh agent picking this repo up cold. State below was verified on
this box on 2026-11-27 (HEAD `1ed5135`, clean tree, remote in sync).

## What this is

bionichat is a local MCP hub for one person's own machine: it merges every MCP
server into one endpoint, shows what the agent is doing live, and can publish the
endpoint through a public tunnel so a remote client (Windows PC, phone agent) can
drive the same machine. Public repo: `github.com/youssefvdel/bionichat`,
AGPL-3.0-or-later, release `v0.1.0` exists.

Two shells exist on purpose:

- `src-tauri/` — the shipped Tauri + CEF app (legacy, works, 43 MB release zip).
- `apps/desktop/` — the Rune app (new face of the product). It renders fake data
  today; the hub is not wired in yet.

## Layout

| path | what |
|---|---|
| `crates/bionichat-core/` | hub, mcp, policy, registry, tunnel — no Tauri, no windowing |
| `crates/bionichat-core/src/hub.rs` | MCP JSON-RPC, tool union, approvals, activity feed, image publishing |
| `crates/bionichat-core/src/lib.rs` | `EventSink` trait (`emit(&str, Value)`) + `hide_window` |
| `src-tauri/src/lib.rs` | Tauri commands; `TauriSink` forwards hub events to the window |
| `apps/desktop/src/main.rs` | the whole Rune UI (single file, ~700 lines) |
| `tools/core/` | one built-in MCP server: `read write edit bash skill` |
| `tools/bash/` | library behind `bash`: detached jobs, log, kill |
| `docs/specs/ui-rune.md` | frozen UI spec (3 lenses, 2 clean rounds) |
| `docs/UI-STACK.md` | why Rune beat Tauri |
| `docs/spike/` | snapshots: `rune-titlebar.png`, `rune-projects.png`, `rune-settings.png` |

## Verified state

- `cargo test -p bionichat-core` → **38 passed, 0 failed, 0.28 s** (fast: the
  test build has no Tauri).
- `cargo build -p bionichat-core` clean.
- `apps/desktop` `cargo build` clean; live window ran on `:0`.
- `.git` is 2.3 MB; history rewritten twice (a 140 MB `dist/` blob, then a
  600 MB `apps/desktop/target/`). `apps/desktop/target` is now ignored and
  untracked — never commit build output.
- Remote `main` == local `main`. Release `v0.1.0` assets are from the Tauri app.

## The Rune app

Pinned fork: Rune `4918969f04dce020f63b34e024368a86e5e1857d` with 8
`[patch.crates-io]` entries in the workspace `Cargo.toml`. Own workspace under
`apps/desktop`. Source checkout for API reading: `/tmp/opencode/rune-src`
(may be gone; re-clone at the pin).

What exists (all fake data):

1. Custom **title bar**: breadcrumb left, `− □ ✕` right, `.window_drag()` band.
2. **Sidebar**: the real logo (4 SVG paths from `assets/logo/bionichat-master.svg`,
   drawn with `Path::from_svg` + `Area` + `Shape`), `Overview`, `Projects`,
   `Settings`, Settings pinned to the bottom.
3. **Overview**: link strip, RUNNING card(s), RECENT activity with mono command
   blocks, SERVERS rail.
4. **Projects** and **Settings**: list rows on the same tokens.

Run it:

```sh
cd apps/desktop
cargo run                      # live window (needs DISPLAY=:0)
cargo run -q -- --snap /tmp/opencode/x.png --scene busy --size 1280x720
cargo run -q -- --tree /tmp/opencode/tree.json --scene idle   # element tree
```

Scenes: `busy` (running + recent), `idle` (no running), `projects`, `settings`.

### Rune traps already paid for

- `.scale(k)` scales about the **centre** and does not clip; anchor it with
  `.origin(0.0, 0.0)` and clip on the parent, or the drawn subtree lands outside
  a small container and looks empty.
- A `row()` needs `.items_center()` (or `.center_y()`) for vertical centring.
- Buttons need `.role("button") .label("…") .focusable(true)` for accessibility,
  and `.no_window_drag()` if they sit inside a drag band.
- Window actions are `on_press_tx(|s, tx| tx.minimize() / tx.toggle_maximized() /
  tx.close_window())`.
- Snapshot text comes from the real font stack; the mono face is used for
  command/output blocks only.

## Next steps, in order

1. **Wire the hub into Rune** — `bionichat-core` is Tauri-free for exactly this.
   Implement `EventSink` for Rune (push events into state, wake the app), start
   `Hub::new(registry_path, port, Some(sink))` at launch, replace the fake
   `State` with a snapshot from `hub.*()` getters.
2. **Projects view real** — folder list from the registry's file roots.
3. **Settings real** — tunnel start/stop, `Revoke`, autostart, paths.
4. **Approval prompt** — the hub emits `approval-requested`; the window has no
   UI for it yet. Nothing can approve an Ask policy in Rune today.
5. **Title bar controls** — rendered and wired, never clicked on the live window.
   Click-test, then claim it.
6. Release path for Rune (bundle, strip, CI) — not started.

## Traps that cost time before

- `pkill -f "…bionichat-desktop"` kills your own shell (the pattern is in the
  shell's cmdline). Use `ps -eo pid=,comm= | awk '$2 ~ /^bionichat-desk/ …'`.
- GUI needs `DISPLAY=:0` and
  `XAUTHORITY=/run/user/1000/selkies-session-tm5gmrs4/Xauthority`.
- Do not run the Vite build and cargo at once in the Tauri app; touch
  `src-tauri/build.rs` if the window is stale.
- `src-tauri/tauri.conf.json` still lists `-p fs -p bash -p skill` in
  `beforeBuildCommand`; the servers are one `core` server now.
- Old hub tests keyed policies by server `fs`/`bash`/`skill`; they now use
  `core`. Keep test server names in step with `registry::default_specs`.

## Style rules the UI already follows

Ground `#161616` (grey-1100), panels `#1a1a1a` (layer-01), alpha hairlines
`#ffffff14`, text `#e6e6e6` / muted `#a0a0a0` / faint `#6b6b6b`, radius 10 panels
and 6 rows, UI text 13 px (medium 530), mono for commands. Taken from OpenCode
Desktop's own theme, not invented.
