# Spec — one core server, background bash

Status: **implemented** (2026-11-26), verified by unit tests and a raw MCP probe.

## Problem

Three built-in helpers (`fs`, `bash`, `skill`) appeared as three servers, each
spawning its own process and shipping its own binary, while their tools all
advertised the `core` namespace. The user sees a list that lies about the
shape of the product, and packaging copies three executables that could be one.

Long shell commands were also capped at 120 s and then killed, so a real build
inside a chat failed with "timed out" and no way to continue.

## Decision

1. **One built-in server.** `core` advertises `read`, `write`, `edit`, `bash`,
   `skill` from a single process and a single binary. Three crates stay in the
   tree as libraries; `tools/core` composes them.
2. **All five tools stay** (user decision, 2026-11-26). Dropping `edit` would
   force whole-file rewrites; dropping `skill` would remove the playbooks.
3. **Background bash.** `bash` gains three fields:
   - `background: true` — spawn detached, return `{job, log, state}` at once.
     No timeout.
   - `job: "<id>"` — return `{state, exit_code?, output}` from the job's log
     and exit file.
   - `job: "<id>", kill: true` — stop it.
   Jobs live in `~/.cache/bionichat/jobs/<id>.{log,exit,pid}`, so they survive
   an app restart and any process can read them. State is files, not memory.
4. **Registry migration.** On startup any built-in whose name is no longer in
   `default_specs` is dropped (the old `fs`, `bash`, `skill` rows), and `core`
   is added or repointed at the binary beside the app.

## Rejected

- Three processes behind one UI row: cheaper, but packaging and process count
  keep the lie, and the migration would never happen.
- In-memory job registry: faster to write, lost on restart, single-process only.
- A separate `bash_output` tool: more names to learn; `job` is one parameter.

## Proof

```
tools/list                      -> ["read","write","edit","bash","skill"]
bash {background:true}          -> {"job":"1a2175b82522da6","state":"running",...}
bash {job:"1a2175b82522da6"}     -> {"state":"done","exit_code":0,"output":"core-bg-ok\n"}
cargo test -p bash              -> 2 passed
cargo test -p fs -p skill -p core -> 3 passed
```

Not yet proven: the packaged app runs `core` end to end on Windows and macOS.
That lands with the Rune build.
