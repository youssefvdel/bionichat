# Platforms

One codebase, three operating systems. What is proven and what is not:

| | Linux | Windows | macOS |
|---|---|---|---|
| build | **proven** (this box) | **proven** (`youssef-pc`) | CI only, never run |
| dashboard | **proven** | **proven** | untested |
| hub `POST /mcp` | **proven** | **proven** | untested |
| `read` / `write` / `edit` | **proven** | **proven** | untested |
| `bash` | `sh -c`, **proven** | `cmd /C`, **proven** | `sh -c`, untested |
| skills | **proven** | **proven** | untested |
| public link, stable hostname | **proven** | **proven** | untested |
| headless, no window | **proven** | **proven** | untested |
| renderer | CEF (v3 alpha) or wry | WebView2 (wry) | WKWebView (wry) |

Linux is the development target. Windows has been built and driven end to end
on a real machine. macOS compiles in CI and ships a folder of binaries, but no Mac has run it,
so `MACOS.md` has the first-run checklist instead of claims.

## Same on every platform

- `bionichat` plus its helpers in one folder: `fs`, `bash`, `skill`,
  `bionichat-tunnel`.
- `bun run build` builds the UI and places the tunnel helper beside the app.
- `bun run package` zips that folder for the current platform.
- The public link is stable per machine: `https://<id>.opentunnel.xyz`.
- `BIONICHAT_TUNNEL_AUTOSTART=1` brings the link up with the app, for machines
  nobody clicks.
- Headless is a setting (Settings → Tunnel: *Open with no window next time*),
  stored in `settings.json`. Proven on Linux and Windows: app running, no
  window, hub answering, the app's own helper serving the stable link.

## Differences that matter

- **Shell**: `sh -c` on Linux and macOS, `cmd /C` on Windows.
- **Window drag**: the title bar moves the window through backend commands,
  because the CEF runtime has no drag API. Works on Linux; on Windows and
  macOS it goes through the same Tauri calls but nobody has dragged it there.
- **Renderer**: Linux defaults to `wry` and can build with CEF
  (`--no-default-features --features cef`); Windows uses WebView2; macOS uses
  WKWebView.
- **Files**: `bash` starts in the user's home (`HOME`, or `USERPROFILE` on
  Windows); data lives in Tauri's `app_data_dir`, which is
  `~/.local/share/com.bionichat.app` on Linux,
  `%APPDATA%\com.bionichat.app` on Windows and
  `~/Library/Application Support/com.bionichat.app` on macOS.
