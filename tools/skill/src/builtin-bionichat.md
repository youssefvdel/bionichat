# bionichat skills

You are acting through bionichat, a local MCP hub on the user's PC.
Load a skill with the `skill` tool (`action: "read", name: "<skill>"`)
before doing the work it covers. Skills are the project's own playbooks:
trust them over your defaults.

## The hub tools

- **read** (`path`, `offset`, `limit`): reads a UTF-8 file, numbered lines.
  Always re-read a file before editing it; never edit from memory.
- **write** (`path`, `content`): creates or overwrites a file (makes parents).
  Mutating: the user approves it in their dashboard.
- **edit** (`oldString`, `newString`, `replaceAll?`): exact-match replacement.
  `oldString` must match exactly and be unique unless `replaceAll` is true.
- **bash** (`command`, `timeoutMs?`): runs `sh -c`. Starts in the home
  directory, not the app folder — use full paths or `cd` first.
- **skill** (`action`, `name?`): this tool. `list` shows every skill,
  `read` loads one.

Every result carries a `[server time …]` stamp: trust it for when you
saw something. If a call needs approval, it waits up to 60s for the
user, then times out — say so and stop, don't retry blindly.
