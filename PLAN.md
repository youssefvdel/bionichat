# bionichat — PLAN (dashboard first, chats later)

Goal: Tauri v3 alpha + Rust app whose first screen is OUR dashboard: tunnel status, available tools, MCP registry. CEF renderer ready for chats later.

## Locked decisions
- Stack: Tauri v3 (alpha.6, pinned) + Rust backend. CEF renderer (bundled Chromium, same pixels everywhere).
- Chats path stays open in-app via CEF. Dashboard first, chats later.
- Alpha accepted knowingly: pin version, expect churn, v3 brings CEF + official MCP plugin.
- Shell owns: MCP registry, supervisor (spawn/health), tunnel helper, approvals log.
- MCP input format: Anthropic format (`{ "mcpServers": { name: {command,args,env} | {url,headers} } }`). Accept paste-JSON + import file (claude_desktop_config.json).
- Name: bionichat (locked 2026-11-20, verified: npm 404, GitHub 0, .com/.dev/.app/.xyz free; neighbor risk LM Studio Bionic noted). Scaffold rename from chatsudo landed 2026-11-20 (crate `bionichat`, lib `bionichat_lib`, identifier `com.bionichat.app`).
- Chats: DEFERRED. No embedded chat windows in v1. Sign-in gate moves with chats.

## v1 scope (dashboard)
1. Dashboard screen: tunnel status (off/starting/ready + public URL), server list (name, transport, status dot), tools list per server.
2. Add MCP: paste Anthropic JSON or import file → validate → save to registry → spawn → health check → tools appear.
3. Hub serves POST /mcp (StreamableHTTP JSON, localhost 127.0.0.1) aggregating all running servers.
4. Tunnel helper: one click runs OpenTunnel route for the hub port, shows public URL + copy button. Demo data only (public URL = public).
5. Approvals UI: per-tool Allow/Ask/Deny (default Ask for exec/write, Allow for read-only demo tools).

## Passes
- P1: Tauri v3 shell + dashboard (servers list, add-via-Anthropic-JSON, tools list). Two demo servers (ping, scoped fs.read).
- P2: hub /mcp aggregator on localhost + OpenTunnel button + status.
- P3: approvals + persistence (registry file, restart supervision). Chats explicitly NOT in v1.
- P4: real tools (read/write/edit/bash) replacing demos, bare names, server-time stamps, handshake briefing.
- P5: Tauri approval gate (ask/session-allow/deny), link revoke with fresh URL on next start.
- P6: live activity feed on the main page.
- P7: data-terminal dark restyle; hub-spur logo concepts (A1 refined + power-b finalists).
- P17a: built-in core servers are not removable (flag in the registry, guard in registry::remove with 3 tests, Remove hidden in the UI).
- P20: packaged folder (`bun run package`), windows CI workflow packs the same five files; Windows run proved the bundled cloudflared end to end (public tools/list + core_bash), zip 25.6 MB.
- P19: bundled tunnel. `scripts/fetch-tunnel.mjs` ships cloudflared beside the app (bun run build), locate() prefers opentunnel for its stable URL and falls back to bundled cloudflared; drain fix keeps the child alive; cloudflared URL filter stops the terms link winning; TunnelInfo.kind shown in Settings. Proved: trycloudflare link served tools and core_bash, Stop killed it.
- P18: Windows build proven on youssef-pc (release exes, hub answers, core_bash via cmd /C); portable bash + tunnel CLI, .exe lookup, WINDOWS.md + windows CI workflow. core_read returns images as MCP image blocks (8 MB cap).
- P17: namespaced tool names (`core_*`, `<server>_*`), prefix routing with no list-first, cached listings (list 4 ms, call 6-11 ms), remove-server command + two-step button, registry parse failures keep a backup instead of reseeding. Chatbot removed.
- P16: in-app chat: project folder, OpenAI-compatible model config, tool loop through the hub, calls rendered as terminal cards. Shell defaults to the project folder; registry self-heals when the app folder moves.
- P15: Projects = folders the user chooses via the in-app picker (browse + Use this folder), saved to projects.json; old ~/Projects listing removed.
- P14: Projects means the user's ~/Projects folders; servers, tools, live map and tunnel move to Settings (pinned bottom); sidebar dimmed with logo footer.
- P13: live tool map on Overview (React Flow): hub → projects → tools, `tool-start` event, animated path, live rail of running and recent calls.
- P12: clay raised (no pressed wells), sidebar with Overview / Projects / Settings, setup moved to Settings, one primary action per screen.
- P11: calm clay — OpenCode's cool neutral tokens (#fbfbfb / #1d1d1f / #007aff, greys from its shell CSS) on soft clay surfaces: 22px panels, layered shadows, pill clay buttons, one primary that follows state. Pointed at point-here (taste-skill, impeccable, React Bits) for the rules.
- P10: calm light theme (achromatic-ledger: paper ground, ink text, blue reserved for live data). Dropped neon green + entrance cascade per design-skill calibration.
- P9: React + Vite + Tailwind UI with custom title bar (logo mark, min/max/close wired through backend commands; native decorations off). Same commands, same gate, same live proof.
- P8: skill manager — `skill` tool lists/loads playbooks from the `.agents` folder (`skills/<name>/SKILL.md`) plus a built-in `bionichat` skill. Read-only.

## SRS (testable)
1. App boots to dashboard in ~1s cold (normal disk).
2. Paste Anthropic JSON with 1 stdio server → validates, spawns, status green, tools listed.
3. Import claude_desktop_config.json with 2 servers → both spawn, both green.
4. Bad JSON / bad command → red status + exact error line, app stays up.
5. GET hub /mcp tools/list from a local client returns union of all running servers.
6. Tunnel button → public HTTPS URL shown, serves /mcp with valid cert.
7. Kill a server process externally → status flips red within 5s, restart button works.
8. Fresh launch restores registry + server states without re-paste.

## Security
- Localhost bind only. Public tunnel URL = public: demo data until auth lands.
- Default Ask for exec/write/shell; Allow only for read-only demo tools.
- Never log secrets (env values masked in UI + logs).

## Research log
- 2026-11-19 web AIs: all 3 take remote MCP over public HTTPS, all block localhost. OpenTunnel YES for demo (blind-TLS, path /mcp passes, ~60s first cert, 15-min idle reset).
- 2026-11-19 shell race: Electron wins heavyweight (mature, login best-tested) but 550-850MB steady → parked for light goal. Bunium spike rendered Gemini unblocked but 560MB RSS + solo + 0.0.7 → killed. Electrobun-CEF 800MB+ beta. Deno desktop CEF experimental, parked. Wails/Lorca no bundled Chromium. Go Energy solo maintainer.
- 2026-11-20 Tauri: v3 is alpha.6 (no beta, no docs) but HAS the CEF renderer + official MCP plugin — user knowingly locks v3 alpha, version pinned. v2 (2.12.7) stays the stable fallback.
- 2026-11-20 scaffold facts (verified against crates.io + crate sources): CEF is not a `tauri` feature — it is the runtime crate `tauri-runtime-cef 3.0.0-alpha.7`, wired with `Builder::default().runtime(Cef::default())`. It pins `cef =154.5.0`, needs **gtk4** and tauri alpha.6; wry needs gtk3, so the two runtimes cannot be compiled together. `libgtk-4-dev` installed on this box for the CEF path. `tauri-build` has no v3 release past alpha.4. MCP crate: `rmcp 3.11.0` (client `().serve(TokioChildProcess)` + `list_all_tools()`, server `ServerHandler` + `stdio()`).

## Status
- P1 DONE 2026-11-20: shell, registry, dashboard, demo servers, tools/list.
- P2 DONE 2026-11-20: hub `POST /mcp` on 127.0.0.1:8787 (initialize, notifications,
  ping, tools/list union (bare names when unique), tools/call routed over live sessions),
  tunnel button + URL scrape + cert poll. `opentunnel 0.6.2` installed user-local
  (`~/.local/bin`), live proof 2026-11-20: public ZeroSSL-verified URL served
  tools/list + tools/call, Stop took the service down. Tunnel state is now
  hydrated from `opentunnel status` at launch.
- P8 DONE 2026-11-21: live activity feed on the main page — every tools/call recorded (server/tool/time/status/result preview/duration), newest first, pushed over tool-activity events. 36 tests green, live proof: public read appeared in the feed.
- P7 DONE 2026-11-21: lockdown. Approvals are the real gate again (Ask defaults restored): Approve once, Until restart (in-memory grant, dies with the app, Deny still wins), Deny. Revoke link burns the route + stops the tunnel; next start mints a fresh URL. Live proof over public tunnel + screenshot.
- P6 DONE 2026-11-21: handshake briefing — hub initialize() tells every agent what bionichat is, whose PC this is, standing full permission, and a tool guide. 32 tests green, live proof over public tunnel.
- P5 DONE 2026-11-21: every hub result carries a server-time stamp (Cairo local + UTC) so web agents are time-aware. 31 tests green, live proof over public tunnel.
- P4 DONE 2026-11-21: real tools. Servers renamed `fs` (read/write/edit) + `bash` (shell); ping/echo deleted, then `list` dropped (bash covers it). Hub forwards real inputSchema so Gemini knows each tool's params. Live proof over public tunnel: write→approve→read numbered, edit→approve, bash→approve→stdout. `read` defaults Allow, rest Ask.
- P3 DONE 2026-11-21: approvals (Allow/Ask/Deny per tool, dashboard banner,
  Approve/Deny buttons, 60s timeout → -32002, deny → -32001 before any spawn),
  supervision (2s probe, auto-restart with 1/2/4s backoff, crash quarantined
  when auto-restart is off so a lazy `tools/list` cannot resurrect it), restart
  button. Live proof: ask→banner→approve→pong; deny→-32001; kill with
  auto-restart on→new PID; kill with it off→red row, stays dead, Restart revives.
- Two bugs found and fixed in review: rmcp `is_closed()` never flips after a
  child dies (supervisor now probes `tools/list` with a 5s timeout), and a cold
  `tools/call` could not know a tool was read-only (hub now caches the
  `tools/list` hints per server).

## Open
- Name: bionichat locked and applied to crate, productName, identifier and window title 2026-11-20. Google sign-in (moves with chats phase).
- Scaffold: DONE 2026-11-20 (P1). Renderer: CEF is now the default on Linux and Windows (proven building and painting on both, 2026-11-22); `wry` stays as the opt-out and as the macOS CI path.
- CEF gate: RESOLVED 2026-11-20 — `tauri-runtime-cef` alpha.7 builds and renders the dashboard (gtk4). 2026-11-21: the full P3 dashboard renders on CEF (chip reads "cef"); no sudo needed via `scripts/run-cef.sh`, which points `CHROME_DEVEL_SANDBOX` at the root-owned helper. Still open: CEF idle RSS vs wry before chats.
