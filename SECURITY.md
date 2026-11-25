# Security

## What bionichat exposes

bionichat gives a chat running elsewhere the ability to read and write files,
run shell commands and load skills on the machine it runs on. Treat the public
link as the credential: whoever has it has that access.

- **No link, no access.** The hub only listens on `127.0.0.1`; the tunnel
  helper is the only way in from outside, and **Stop** closes it.
- **Revoke burns the route.** Revoke deletes the local route and mints a new
  host on the next Start, so a link that leaked cannot be used again.
- **Per-tool policy.** Allow / Ask / Deny per tool in Settings → Servers, and
  built-in servers cannot be removed (added ones can).
- **No secrets are stored by the app.** The registry, settings and policy live
  in the platform app-data directory; no tokens are written by bionichat.
- **Every result is stamped** with local time and UTC so a model cannot reason
  about "now" from stale file timestamps.

Known limits, stated plainly:

- The link is unlisted, not authenticated. There is no per-user login yet.
- `core_bash` is a real shell with the app's privileges. Deny it if you only
  want file access.
- Images are served from `/files/<token>/<name>` while the app is running; the
  token is random and the store is in memory only.

## Reporting

Open a private advisory on GitHub:
<https://github.com/youssefvdel/bionichat/security/advisories/new>

or email the address in the repository profile. Please include the version
(the app shows it in the sidebar), platform, and the steps you used. Expect an
answer within a few days.
