# Spec — bionichat UI on Rune

Status: **draft, round 0** (2026-11-26)

Step 1 result: Rune `examples/hello` built in 2m57s on this box, rendered
headless (`--snap` → 5,956-byte PNG, GPU init 2.1s, one frame, 12 instances).
Debug binary 191 MB; release size to be measured.
Supersedes `docs/specs/ui-rebuild.md` **as an implementation plan**. That file
stays as the design source of truth: problem statement, tokens, anatomy,
states, trust copy and the CEF-era checks all still apply. What changes is how
they are built.

## P0 — Decision

Build the desktop UI with **Rune** (`anomalyco/rune`) instead of Tauri + CEF +
React. The rest of bionichat — hub, policy, tunnel, tool binaries, MCP surface —
does not change.

Why the change, with facts checked 2026-11-26:

| fact | value |
|---|---|
| Rune | MIT, created 2026-10-22, 8,125★, pre-release, git rev + 8 `[patch.crates-io]` entries |
| renderer | native GPU (wgpu), own text + Taffy layout + springs; **no browser** |
| platforms | macOS, Windows, Linux, iOS, web |
| proof at scale | `anomalyco/opencode-rune` (the app in daily use here): 1,427 Rust files, 383,893 lines, releases today |
| its installers | AppImage 75 MB, deb 56 MB, rpm 73 MB (linux-x64); macOS dmg; Windows setup crate |
| headless | `--snap`, `--tree`, `--script`: PNG, element tree, scripted input |
| a11y | accesskit nodes, `role/label/checked`, live regions |
| ours today | Tauri v3 alpha + CEF: 365 MB zip, Chromium 154 inside, React/Tailwind |

Rune removes the browser runtime, gives one language across shell and backend,
and makes every UI proof a PNG + element tree instead of a screenshot + CDP.

Rejected: staying on Tauri+CEF (365 MB, alpha, two stacks); wry (small but
WebKitGTK/WebView2 divergence, still two stacks); Slint/egui (no text/layout
story close to Rune's, no headless proof of this shape); keeping React and
porting later (the UI is not built yet, so there is nothing to preserve).

### What carries over, what dies

- Carries: every visual decision in `ui-rebuild.md` (palette, type ramp,
  density, anatomy, states, copy, trust surface, acceptance checks).
- Dies: React, Tailwind, shadcn/Base UI, `@xyflow/react`, `motion`, Biome,
  Vitest/Testing Library, Bun as the UI build, Tauri, CEF.
- Stays: Rust workspace, `bionichat` lib (hub/policy/tunnel), `tools/*` MCP
  servers, `scripts/conformance.ts`, tests.

## What changes in the backend — mostly nothing

The MCP surface is the product contract, so it does not move: same tools, same
names (`core_*`), same instructions, same server-time stamp inside the payload,
same image URLs, same `-32001/32002/32602` codes.

| area | change |
|---|---|
| `crates/bionichat` (hub, policy, registry) | **unchanged** except the two named additions |
| `tools/fs`, `tools/bash`, `tools/skill`, `tools/tunnel` | **unchanged** |
| `scripts/conformance.ts` | **unchanged**, must stay 19/19 |
| `src-tauri` (commands, events, setup) | **deleted** — it is the Tauri adapter, not the product |
| hub lifecycle | started in-process by the Rune app instead of Tauri `setup`; same port 8787 |
| events | `HubEvent` over an `mpsc` channel to the view instead of Tauri `emit` |
| `origin` + `ready (external)` | the two small additions already in step 8 |
| packaging | Tauri bundler out; AppImage/deb/rpm/dmg/MSI in |

So: the backend keeps its logic and its contract; it loses the Tauri adapter and
gains a plain Rust handle. If any of the above turns out to be false while
building, that is a plan defect and gets written down here.

## Architecture

```
apps/desktop      Rune app: window, views, tokens, map, rail      (new crate)
crates/bionichat  hub, policy, registry, tunnel, chat-free state  (exists)
tools/{fs,bash,skill,tunnel}                                      (exists)
```

One process. The Rune app calls the hub directly — no IPC bridge, no
`invoke`, no events over a channel boundary. A background task feeds UI state;
`Live<T>` / `LiveText` update at frame rate without rebuilding the view, and
`App::wake()` tells the shell to redraw when a tool call lands.

State lives in one struct owned by the view:

```rust
struct State {
    view: View,              // Overview | Projects | Settings
    servers: Vec<ServerUi>,  // name, builtin, tools, policy
    tunnel: TunnelUi,        // off | ready | ready(external) | error
    live: LiveRoster,        // running calls (LiveText), last 50 finished
    intro: bool,             // first-run block
}
```

## Tokens — same values, Rust constants

Port `ui-rebuild.md` verbatim into `apps/desktop/src/tokens.rs`:

- ground `#0e1014`, panel `#14171c`, raised `#191d23`, border `#23272f`,
  border-strong `#2e333c`, text `#e7e9ee`, muted `#9aa1ad`, faint `#8f97a5`,
  accent `#4c9dff`, ok `#3ecf8e`, warn `#d8a04a`, danger `#ff6b76`.
- radii 12 / 8 / 6; spacing 4 / 8 / 12 / 24; rows 32 / 28; strip 40; rail 320;
  sidebar 224.
- type: 11 / 12 / 13 / 15 / 19 at weights 400 / 550 / 600; mono for URLs,
  durations, tool names, counts, paths.
- motion: `motion::SNAPPY` for state, `0.4/0` for overlays, no bounce.

Fonts: Rune accepts registered families (`AppSpec::fonts`, `Family::Named`).
Ship **Geist** + **Geist Mono** TTFs. Open question with a check: the npm
`@fontsource-variable/*` packages carry woff2; take the OFL TTFs from Vercel's
`geist` repo instead, and prove a snapshot renders them before styling anything.

## Screens

Same three views and the same content as `ui-rebuild.md`. The pieces that need
Rune-specific design:

**Map.** A clipped canvas region with a `Shape` layer under the nodes.
Wires are `Shape::new().stroke_with(points, false, 1.0, LineCap::Round, colour)`
— verified present in `rune-core` and used by `examples/motion-bento`. Node
positions are computed in Rust from the server/tool layout (no React Flow):
hub on top, servers in rows, tools under each. The live path is a second stroke
in `accent`, dashed by drawing segments, and pulses through `Live<f32>`.
Zoom/pan: v1 is `fit` + list toggle only; the map is not a spatial editor.

**Rail.** `LiveText` per entry for the running state (duration ticks, status),
so a 4 s call does not rebuild the view every frame. Finished entries are plain
els; 50 kept in memory, cleared on restart.

**Sidebar / strip / cards.** Plain `row`/`col`/`el`/`text` with tokens. Focus
rings and keyboard order are `.focusable(true)` plus tab order; every control
carries `role` and `label`.

**Accessibility.** Verified API surface: `role`, `label`, `checked`,
`focusable`, `LiveRegion::{Polite, Assertive}`. The approval dialog announces
assertively, the link state politely, and the map list is a real list of rows.

## Headless proof — the reason this stack wins

Every state is reproducible without a person:

```bash
cargo run -p bionichat-desktop -- --snap shots/overview.png --scene ready
cargo run -p bionichat-desktop -- --tree --scene busy | rg 'core_bash|running'
cargo run -p bionichat-desktop -- --script scripts/ui/approve.json --snap shots/approval.png
```

Scenes: `first-run`, `idle`, `busy`, `offline`, `external`, `error`,
`quarantine`, `many-servers`, `long-output`.

## Steps, each with its check

1. **Toolchain spike.** Clone Rune at a pinned rev; build `examples/hello` and
   render it headless. Check: `--snap` writes a PNG, `--tree` prints the switch
   with role/label, `cargo build` exit 0.
2. **App crate skeleton.** `apps/desktop` with a window, tokens, sidebar, strip,
   and three empty views on fake state. Check: `--snap` of each view; no
   clipping at 1280×720; `cargo clippy` clean.
3. **Fonts and type.** Register Geist + Geist Mono TTFs, set the ramp. Check:
   snapshot shows both faces; a mono URL and a tabular count align.
4. **Hub wiring.** Depend on `crates/bionichat`; replace the Tauri `AppHandle`
   event sink with `HubHandle::start(..) -> (HubHandle, mpsc::Receiver<HubEvent>)`
   and start it in-process on 8787; feed servers/tools into state. This is the
   one structural refactor, and it touches no protocol behaviour. Check: real `tools/list` fills the
   sidebar; killing a server shows it quarantined within 5 s.
5. **Map.** Layout hub→server→tool, draw wires, highlight a live call. Check:
   snapshot with a real `core_bash` running shows the accent path; idle shows
   no accent; 50-tool scene still renders in one snapshot.
6. **Rail.** Cards with command/output, `local`/`link`, durations via LiveText.
   Check: a 4 s call shows running → ok without rebuilding the view (assert the
   frame counter in `--tree`), 50-entry cap proven.
7. **Projects and Settings.** Folder picker (Rust `rfd` or an in-app browser),
   Link card with blast radius and the two-step Revoke, servers with per-tool
   policy, approvals dialog, files block. Check: policy persists to
   `registry.json`; Deny returns `-32001`; Copy yields `…/mcp`.
8. **Backend additions.** `origin` from the `Host` header (threaded through
   `post_mcp` → `handle_rpc` → `route_call`), and `ready (external)` from the
   profile hostname + lock probe + POST ping. Check: tests plus one public call
   marked `link`.
9. **Motion and reduced motion.** Rune springs for state changes; honour the OS
   setting (verify the API before wiring; if absent, a setting in
   `settings.json` and a flag). Check: with reduce on, no spring travels.
10. **Packaging.** One crate per target, following the shape proven by
    opencode-rune: AppImage + deb + rpm on Linux, Windows setup, dmg on macOS.
    Check: the **installed** artifact launches, shows Overview, runs a call.
11. **Parity and removal.** Conformance 19/19; the three data files unchanged;
    then delete the React tree and the Tauri crate in one commit, before the
    first release on Rune is tagged.

### Risks

| risk | likelihood | mitigation |
|---|---|---|
| Rune API churn (pre-release, git rev) | high | pin one rev in `Cargo.toml`; bump deliberately, never "latest" |
| Geist TTFs unavailable or unrenderable | medium | fall back to Rune's Inter + IBM Plex Mono, and say so |
| Map drawing cost at 50+ tools | medium | fit-only v1, collapse per server, list fallback |
| Windows packing with wgpu/newer Rust | medium | build on youssef-pc early, before the UI is finished |
| macOS untested | known | CI build only; say it in the release notes |
| Binary/platform size worse than expected | low | measure step 1 and step 10; report the number |

## Definition of done

Every check above passes with quoted output; snapshots for first-run, idle,
busy, approval and error are committed; the installed Linux package runs on
`:0` and completes a real call; conformance 19/19; `registry.json`,
`settings.json` and `projects.json` hashes unchanged.
