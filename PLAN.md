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
  ping, tools/list union as `server__tool`, tools/call routed over live sessions),
  tunnel button + URL scrape + cert poll. `opentunnel 0.6.2` installed user-local
  (`~/.local/bin`), live proof 2026-11-20: public ZeroSSL-verified URL served
  tools/list + tools/call, Stop took the service down. Tunnel state is now
  hydrated from `opentunnel status` at launch.
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
- Scaffold: DONE 2026-11-20 (P1). Default runtime is `wry` so a fresh clone builds without the CEF download; `--features cef` is wired and tested separately. Flip the default once CEF is confirmed on the demo box.
- CEF gate: RESOLVED 2026-11-20 — `tauri-runtime-cef` alpha.7 builds and renders the dashboard (gtk4). 2026-11-21: the full P3 dashboard renders on CEF (chip reads "cef"); no sudo needed via `scripts/run-cef.sh`, which points `CHROME_DEVEL_SANDBOX` at the root-owned helper. Still open: CEF idle RSS vs wry before chats.
