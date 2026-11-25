# Contributing

Issues and pull requests are welcome. Keep changes small and explain the
"why" in the commit message.

## Build

```bash
bun install
bun run build                 # frontend (Vite → dist/, gitignored)
cargo build -p bionichat      # app (CEF is the default renderer)
```

CEF needs `cmake` and `ninja` (plus MSVC on Windows). First build downloads
Chromium once; afterwards it is cached in `target/`.

After changing anything under `src/`, touch `src-tauri/build.rs` before
building the app: the frontend is embedded at compile time and cargo does not
notice `dist/` changed.

## Checks before a pull request

```bash
bun run check                 # tsc --noEmit
cargo test                    # hub, registry, policy, docx
cargo clippy --workspace --all-targets -- -D warnings
bun scripts/conformance.ts <public-url>/mcp   # black-box, needs a live link
```

The conformance suite speaks raw MCP over HTTPS and does not import app code;
it is the test that proves a release, not a claim about it.

## Style

- Rust: `cargo fmt`, clippy clean, no `unwrap` on paths a user can reach.
- TypeScript: strict, no `any` in new code, Tailwind tokens from `src/index.css`
  instead of raw hex.
- Commit subjects: `area: what changed`, imperative, under 72 characters.

## Security

Do not open a public issue for a vulnerability. See [SECURITY.md](SECURITY.md).
