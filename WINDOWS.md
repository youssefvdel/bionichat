# Windows

Short answer: the app is built to run on Windows, and the parts that were
Linux-only are fixed. What is missing is a Windows build, because a Tauri app
cannot be cross-compiled from this Linux box.

## What already works on Windows

- Tauri shell, the dashboard (React) and the hub (axum) are plain
  cross-platform code.
- `tauri-runtime-cef` declares `x86_64-pc-windows-msvc` as a supported target,
  so the Chromium renderer is available; `wry` (WebView2) is the default and
  works out of the box.
- The `fs` and `skill` servers are `std::fs` only. Absolute Windows paths pass
  straight through; relative ones join the base and reject `..` escapes.
- Data lives in Tauri's `app_data_dir` (`%APPDATA%\com.bionichat.app`), so the
  registry, projects and skills follow the platform.

## No console windows

The app is a GUI program but every child it starts (the four tool servers and
the tunnel helper) is a console program, and Windows gives each one its own
console window. They are now spawned with `CREATE_NO_WINDOW`, so the app opens
as one window with no terminals behind it.

Verified on `youssef-pc`: enumerating the desktop's visible top-level windows
shows `bionichat` and no console windows; the four child processes report no
windows. (Do not trust `Process.MainWindowHandle` for a window started by the
Task Scheduler: it reads 0 even when the window is visible.)

## What was Linux-only, now fixed

- `bash` ran `sh -c`; Windows has no `sh`, so it now runs `cmd /C` there and
  keeps `sh -c` everywhere else.
- The tunnel no longer needs the upstream CLI. That CLI refuses to build
  outside Unix, but the library under it is portable, so bionichat ships
  `bionichat-tunnel` (tools/tunnel), a foreground client that prints the
  public URL and serves it. Same stable hostname per machine, no service
  manager, no sockets: `bun run tunnel:build` builds it and puts it beside the
  app.

## Proven on Windows (22 Nov 2026)

The 22 Nov build shipped with the tunnel inside and was proven end to end:

- app running in the desktop session, hub on `127.0.0.1:8787`
- the bundled helper minted its stable hostname
  `https://<id>.opentunnel.xyz`
- over that public URL: `tools/list` returned the five `core_*` tools and
  `core_bash` returned `public-windows-ok` with `C:\Users\Administrator`
- one folder, zipped: `C:\Users\Administrator\bionichat-windows.zip` (25.6 MB)


Built on the Windows PC with `cargo build --release` and run there:

- The app starts and opens its window in the desktop session.
- The hub answers on `POST http://127.0.0.1:8787/mcp` with all five `core_*`
  tools and their schemas.
- `core_bash` ran through `cmd /C` and returned `win-ok` plus
  `C:\Users\Administrator`, the user's home, with the server clock attached.

Artifacts: `C:\Users\Administrator\bionichat-win\` holds `bionichat.exe`
(13.5 MB) with `fs.exe`, `bash.exe` and `skill.exe` beside it, which is exactly
the layout the app expects.

Two things that bit during that first run, worth knowing:

- Launching the exe over SSH fails with `WebView2 error: Invalid window
  handle`, because SSH runs in session 0 where Windows will not create a
  window. It must run in the logged-on desktop session.
- Screenshots of that session came back blank while the display was asleep, so
  the visual check was done by talking to the hub instead.

## What is still unproven on Windows

- The **CEF** renderer: it declares MSVC support, but nobody has run it here.
  The realistic first run is WebView2 (`wry`), which needs the WebView2 runtime
  (preinstalled on Windows 10/11).
- **The tunnel helper on Windows**: it cross-compiles for Windows, but the
  Windows run of the tunnel itself is untested.
- Window drag: the custom title bar moves the window by asking the backend for
  a new position. That is Windows-safe (`set_position`), but only the Linux
  path has been exercised.

## One folder, nothing installed

`bun run tunnel:build` puts the tunnel helper beside the app binary, so a
Windows build folder contains everything:

```
bionichat.exe      the app
fs.exe             file tools
bash.exe           shell tool  (cmd /C on Windows)
skill.exe          skill loader
bionichat-tunnel.exe  the tunnel, stable hostname, nothing to install
```

Zip that folder (`bun run package`, or `Compress-Archive`) and it runs on any
Windows 10/11 machine as-is.

## How to get a build

Two honest routes:

1. **GitHub Actions** (`.github/workflows/windows.yml`): push the repo, run the
   workflow, download `bionichat-windows-<version>`. It builds the UI, the
   three tool servers and the app on `windows-latest`, then uploads the `.exe`
   set as one folder: `bionichat.exe` plus `fs.exe`, `bash.exe`, `skill.exe`
   beside it (the app looks for its tools next to itself).
2. **Build on the Windows machine itself**: install Rust (MSVC toolchain),
   Bun, and the Visual Studio Build Tools, then:

   ```powershell
   bun install
   bun run build
   cargo build --release --workspace --exclude bionichat
   cargo build --release --manifest-path src-tauri/Cargo.toml
   ```

   Run `src-tauri\target\release\bionichat.exe` with the three tool `.exe`s in
   the same folder.

Installer packaging (NSIS/MSI) is not wired yet: `tauri.conf.json` has no
bundle targets for Windows, so the artifact is a folder, not a setup wizard.
