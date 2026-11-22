# macOS

Status: **the code is written for macOS and CI builds it, but nobody has run
it on a Mac yet.** Everything here says what is covered and what is not, so the
first Mac run has a checklist instead of a surprise.

## What is shared with Linux (already proven)

- The hub, the dashboard (React) and the four tools are plain cross-platform
  code; the tools are `std` + `tokio` only.
- The tunnel helper (`bionichat-tunnel`) is built from the upstream OpenTunnel
  library, which compiles on macOS. It stores its identity the same way, so a
  Mac gets its own stable `https://<id>.opentunnel.xyz`.
- `bash` uses `sh -c` on Unix, so macOS behaves exactly like Linux.
- Data lives in Tauri's `app_data_dir`, which maps to
  `~/Library/Application Support/com.bionichat.app` on macOS.

## What is mac-specific and unproven

- **Window chrome.** macOS uses `wry` (WKWebView) by default. The custom title
  bar moves the window through backend commands (`window_spot`/`window_move`)
  because the CEF runtime has no drag API. On macOS the same commands go
  through Tauri's `set_position`, which should work, but the traffic-light
  buttons (red/yellow/green) are not implemented: the app paints its own
  minimise/maximise/close on the right like Windows.
- **No `.app` bundle yet.** CI ships a folder with the binary and its helpers.
  A proper bundle (icon, Info.plist, Gatekeeper signature) needs
  `tauri build`, which is not wired for this project yet.
- **Gatekeeper.** An unsigned binary downloaded from the internet is blocked
  until it is allowed in System Settings → Privacy & Security. Builds from CI
  hit this too.

## How to get a build

CI job `desktop` → `macos` (`.github/workflows/windows.yml`) on
`macos-latest`:

```
bun install && bun run build          # UI + tunnel helper
cargo build --release --workspace --exclude bionichat
cargo build --release --manifest-path src-tauri/Cargo.toml
bun run tunnel:build
```

The artifact is `bionichat-macos.zip` with `bionichat`, `fs`, `bash`, `skill`
and `bionichat-tunnel` in one folder. Unzip, then run `./bionichat` from that
folder (it looks for its helpers beside itself).

On the Mac itself the same commands work with Homebrew Rust + Bun installed.

## First-run checklist for a Mac

1. `./bionichat` opens a window with the dashboard.
2. Settings → Servers shows `fs`, `bash`, `skill` as ready.
3. Add a server from Anthropic JSON works.
4. Settings → Tunnel → Start tunnel shows `opentunnel ready` and a
   `https://<id>.opentunnel.xyz` link that answers `tools/list` from the
   internet.
5. Stop kills the link; Revoke replaces the hostname.
