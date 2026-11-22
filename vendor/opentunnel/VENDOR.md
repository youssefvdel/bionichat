# Vendored: OpenTunnel client library

Upstream: https://github.com/anomalyco/opentunnel (MIT), pinned at commit
`b1a27e3df30a25ae1caa0d3a30b2d7ff9585b033` (crate `opentunnel` 0.6.2).

Why vendored: the upstream CLI is Unix-only (`compile_error!` on other
platforms) because its background service speaks over a Unix socket. The
client library itself is portable, so bionichat ships its own small tunnel
binary (`tools/tunnel`) built on this crate instead of depending on the CLI.

Do not edit `src/`: re-copy it from upstream to upgrade, and keep this note.
The only change made here is `Cargo.toml`, rewritten from workspace
inheritance to concrete values so the crate builds outside that workspace.
