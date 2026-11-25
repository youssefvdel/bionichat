# Changelog

Every user-visible change, newest first. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-11-25

First release. Linux and Windows 11 proven; macOS builds in CI but is untested
by hand.

### Added

- Local MCP hub over `POST /mcp`, one endpoint for every connected chat:
  `initialize` with a server description that names the user's own PC, tool
  listing, tool calls, sessions, and `-32602` for unknown tools.
- Built-in tools: `core_read` (numbered text, images as image blocks,
  PDF/docx/xlsx as text, 8 MB image and 32 MB document limits), `core_write`,
  `core_edit` (exact match, `replaceAll`), `core_bash` (real shell, `cmd /C`
  on Windows, `$HOME` default), `core_skill` (`.agents/skills/<name>/SKILL.md`).
- Any extra MCP server can be added in Anthropic's `mcpServers` JSON shape and
  removed again; tools are namespaced `<server>_<tool>` and listings are
  cached so a slow server never stalls a chat.
- Public link without an account: bundled OpenTunnel helper (vendored, MIT)
  gives a stable `https://<id>.opentunnel.xyz/mcp` per machine; Start, Copy,
  Stop and Revoke from the app, and the helper dies with the app.
- Dashboard: Overview with a live map (hub → server → tool, lit while a call
  runs) and a live calls rail; Projects as folder cards with an in-app folder
  picker; Settings for the link, servers, per-tool Allow/Ask/Deny, and the
  files the app keeps.
- Every tool result ends with a `[server time …]` stamp in local time and UTC
  so the model knows when it acted.
- Full tool-result previews in the rail: shell commands with output, reads with
  numbered lines, writes and edits with their path.
- One-folder packages: `bun run package` builds a zip that carries the app,
  the tool servers, the tunnel helper and Chromium's runtime. Unzip and run.

### Fixed

- Windows shell: quoted commands reached `cmd.exe` mangled; the raw command is
  now passed through, and the conformance suite asserts exit codes.
- The public endpoint is shown and copied with `/mcp`, not the bare host.
- Embedded frontend: the UI is compiled into the binary, so release builds
  touch the Tauri build script after a UI change instead of shipping the
  previous screen.
- Package script: a CEF runtime directory is matched by name, never by a stray
  `libcef.dll` inside cargo's build tree; staging moved out of `dist/`.
- Built-in tool servers follow the binary beside the running app, so a fresh
  unzip cannot spawn an older `fs.exe`.
- `.docx` tables keep their tabs (`<w:tab/>` is an empty XML element).
- Excel workbooks stop at the text clamp instead of materialising every cell.
- The packaged Linux app aborted on first launch ("SUID sandbox helper binary
  was found, but is not configured correctly"): a zip cannot carry a setuid
  helper, so the app now detects that and starts Chromium with `--no-sandbox`,
  logging one warning. `SECURITY.md` says how to restore the sandbox.

### Security

- The public link is the trust boundary: anyone holding it can call the tools.
  Deny per tool, Revoke the link, or Stop the tunnel from Settings. The link is
  unlisted but not secret, and stopping the app closes the door.
