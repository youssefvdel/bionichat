# Handoff — bionichat, 26 Nov 2026

Read this first, then `PLAN.md` (section **P13**) and `README.md`.
Written for a fresh chat with no memory of the work that produced it.

## What this is

bionichat runs a small MCP hub on the user's own machine and gives a web chat
(ChatGPT, Claude, Gemini, OpenCode, anything speaking MCP) his real tools over
one public HTTPS endpoint: `core_read`, `core_write`, `core_edit`, `core_bash`,
`core_skill`.

- Repo: https://github.com/youssefvdel/bionichat (public, AGPL-3.0-or-later)
- Release: `v0.1.0`, Linux + Windows zips proven by hand, macOS built in CI but
  **never launched by hand**
- Stack: Rust (Tauri v3 alpha.6 + `tauri-runtime-cef` alpha.7, bundled Chromium),
  React 19.3.1, Vite 8.3.8, Tailwind 4.3.5, TypeScript 7.0.2, `@xyflow/react`
  13.0.3
- ~80 commits, CI `desktop.yml` (Windows + macOS jobs)

## State that is proven

- Conformance suite: `bun scripts/conformance.ts <url>` — 19 checks, 19/19 over
  the public link and against `http://127.0.0.1:8787/mcp`.
- Image results publish a public URL (`/files/<token>/read.png`) and travel as a
  native MCP image block first, text second.
- Word/Excel/PDF read as text in `core_read` (no Python): `.docx` via zip +
  quick-xml, `.xlsx`/`.xls` via calamine, PDF via pdf-extract with a 60k clamp.
- Linux zip runs from a clean extract after the sandbox fix: the app detects a
  non-setuid `chrome-sandbox` and starts Chromium with `--no-sandbox`, printing
  one stderr warning (see `SECURITY.md`).

## Decisions worth not re-litigating

| decision | why |
|---|---|
| Tauri v3 alpha + CEF | user required one renderer on every OS; v3 ships the CEF runtime and the setuid helper in deb/rpm |
| OpenTunnel vendored as `tools/tunnel` | upstream CLI is Unix-only; the library is portable; stable per-machine URL |
| Tool names `core_*` and `<server>_<tool>` | no bare names, no `__`; prefix routes without a list first |
| Every result stamped `[server time …]` | models reason about "now"; stamp inside the same block as the payload |
| No approval gate by default | web chats time out; Deny per tool and Revoke the link are the brakes |
| AGPL-3.0 + reserved name | user wants the code protected from resale; `NOTICE` reserves the name |

## Next work, in order

### 1. UI rebuild (the active task, planned)

Goal: an interface that looks designed, not generated. Decisions already
researched and recorded in `PLAN.md` P13:

- shadcn/ui CLI 4.23.1, **Base UI** primitives (`@base-ui/react` 1.9.0; Radix is
  no longer the default), preset `nova` (Geist + Lucide)
- Tailwind 4.3.5 CSS-first tokens, `motion` 14.14.0, `lucide-react` 1.81.0
- Geist Sans/Mono self-hosted via `@fontsource-variable/*`
- Keep `@xyflow/react` for the live map

Rules for this pass: dark, dense, one accent, real focus rings, 8px grid,
motion only where it explains state, reduced-motion honoured, DOM measurements
as proof (not screenshots). Write the spec first, run a fresh-eyes review
(`egm-review` skill or a subagent given only the spec), then build.

### 2. Installers

`tauri build --bundles …` in v3 already ships CEF: `libcef` beside the exe on
Windows, under `/usr/share/<product>` in deb/rpm, framework + five helpers in
the macOS `.app`; deb/rpm install `chrome-sandbox` 4755.

- Targets: NSIS (Windows), deb + rpm + AppImage (Linux), dmg/.app (macOS)
- Blocker to solve: tool servers (`fs`, `bash`, `skill`) and `bionichat-tunnel`
  are resolved **beside the executable**. Installed layouts put them in the
  resource dir, so resolution must learn `resource_dir()` too.
- Test installed, not only unzipped. Signing costs are a user decision
  (Windows cert, Apple $99/yr).

### 3. Smaller items

- Rebuild the Windows zip from current source (it predates the sandbox fix and
  earlier UI commits; behaviour is unchanged there).
- macOS: launch the CI artifact by hand once.

## Environment facts that will bite

- **One build per tree.** Running Vite while cargo reads `dist/` produces a
  phantom "couldn't read dist/<hash>" error. Serialise.
- The frontend is embedded at compile time: `touch src-tauri/build.rs` before
  `cargo build` after any `src/` change, or you ship the previous UI.
- CEF build needs `cmake` + `ninja`. First build downloads ~1 GB into `target/`.
- Desktop: only `DISPLAY=:0` (Selkies). No Xvfb.
  `XAUTHORITY=$(ls -d /run/user/1000/selkies-session-*/Xauthority | head -1)`.
- Launch: `./scripts/run-cef.sh`. App data: `~/.local/share/com.bionichat.app/`
  (`registry.json`, `settings.json`, `projects.json`, `tunnel.log`).
- One instance per machine: app lock on 8788, tunnel helper lock on 8789. A
  second launch exits on purpose.
- The hub listens on `127.0.0.1:8787`, `POST /mcp`. The tunnel helper is the
  only way in from outside.
- Windows box `youssef-pc` is reachable over SSH; the app must run in the
  logged-on desktop session (a Scheduled Task), never in SSH session 0.
- Built-in tool servers follow the binary beside the running app; user-added
  servers keep their own command.

## Verification recipes

```bash
bun run check                      # tsc --noEmit
cargo test --lib                   # hub, registry, policy, docx
cargo clippy --workspace --all-targets -- -D warnings
bun run build && touch src-tauri/build.rs && cargo build -p bionichat
bun scripts/conformance.ts http://127.0.0.1:8787/mcp
bun run package                    # zip into dist/, then extract and run it
```

## How to start in the new chat

1. Say: "read docs/HANDOFF.md and PLAN.md P13".
2. Write the UI spec to a file before touching `src/`.
3. Run the fresh-eyes review, then implement in small verified passes.
