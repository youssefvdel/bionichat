# Spec — bionichat UI rebuild

Status: **draft, round 1** (round 0 reviewed by three fresh-context lenses)
Date: 2026-11-26
Owner: this session

## P0 — Problem

The dashboard works and looks generated. The review of the current code named
the cause precisely: **sameness**, not colour.

- One surface treatment for everything: panel, row, button and field all share
  `#1e2129`, the same inset highlight and the same drop shadow.
- One typeface at one weight for every role; counts, URLs, labels and headings
  look alike.
- One gap value (`gap-4` / `gap-3`) everywhere, so nothing groups.
- `Pill` + `Panel title= right=` repeated across views.
- A stock React Flow dotted canvas with default nodes.

Three restyles (green, warm paper, claymorphism) failed because they changed
hues and left hierarchy, type character and density untouched. The fix is
structural: distinct surfaces, a real type system, deliberate density, and
bespoke detail in the two places a stranger looks first (the map and the rail).

Users: Youssef, then anyone running the app who connects a web chat.

Goal: an interface that reads as intentionally designed at first glance, with
every state legible and no functional regression.

### Non-goals

- No chat UI and no new features. Two backend changes are in scope, both so the
  UI stops lying: activity entries gain `origin` (`local` / `link`), and the app
  can detect a helper it did not start (`ready (external)`). No other tool
  behaviour changes.
- No installer, macOS or Windows rebuild work.
- No user-facing theming; one theme, done well.
- No marketing page.

### Constraints

- One renderer: CEF, pinned by `cef 154.5.0+154.0.34` = **Chromium 154.0.8037.98**
  (verified in `Cargo.lock`). wry/WebKitGTK is the macOS CI path. No Chrome-only CSS.
- The frontend is embedded at compile time: touch `src-tauri/build.rs` before
  every `cargo build` that follows a `src/` change.
- 1280×720 floor, no horizontal body scroll.
- Offline: no network fonts, no CDN.
- Keyboard reachable, visible focus, reduced motion honoured.
- TypeScript 7: `baseUrl` is removed (TS5102). Alias uses `paths` only, plus the
  Vite `resolve.alias`.

### Assumptions and cost if wrong

| assumption | cost if wrong |
|---|---|
| Base UI 1.9 + React 19.3 + TS 7 typecheck together | **proven in spike 2026-11-26**: 7 components, tsc 0, build ok |
| `shadcn init -b base -p nova` works in this Vite+TW4 repo | **proven in spike**: detects Vite, Tailwind v4, alias, writes components.json |
| Motion 14 honours `prefers-reduced-motion` in CEF | hand-written CSS only |
| The current Tauri commands cover every view state | small backend additions, separate spec |
| Tailwind 4.3 renders the same in WebKitGTK | macOS-only fixes later |

### Kill criteria

- If Base UI cannot typecheck with TS 7 after one attempt, stop; do not ship a
  half-migrated tree. (Spike says this risk is retired.)
- If the new UI cannot be proven in CEF from the built binary, it is not done.

## Visual direction — decided here, not left to the builder

**Surface language: flat, not modelled.** The clay bumps are what read as
generated. Surfaces are flat fills with a 1px border. Shadow is reserved for
things that float (dialog, popover, toast) and appears nowhere else.

| token | value |
|---|---|
| ground | `#0e1014` |
| panel | `#14171c` |
| panel raised | `#191d23` (rows, inputs) |
| border | `#23272f` |
| border strong | `#2e333c` |
| text | `#e7e9ee` |
| muted | `#9aa1ad` |
| faint | `#8f97a5` (4.6:1 on ground; never lighter for text) |
| accent | `#4c9dff` (the only saturated colour that is not a status) |
| ok | `#3ecf8e` |
| warn | `#d8a04a` |
| danger | `#ff6b76` (text and rings; `#e0555f` only as a 14% fill) |

Radii: 12px panels, 8px controls, 6px chips. No `rounded-2xl`, no pills-as-rows.

**Type system: three voices.**

- Geist Sans, 13px, weight 400 — body and rows.
- Geist Sans, 11px, weight 550, letter-spacing 0.06em, uppercase — section
  labels and table headers only.
- Geist Mono, 12px — URLs, durations, tool names, code, counts.
- Page titles: 19px, weight 600, tracking -0.02em. Nothing larger.
- `font-variant-numeric: tabular-nums` on every number that can change.
- Weights used: 400, 550, 600 only. No italics.

**Density: an 8px grid with two row heights.** 32px for interactive rows, 28px
for meta rows. Panel padding 16px, gap between groups 24px, inside a group 8px.
Nothing uses `gap-4` by habit.

**The map is the signature.** Dark canvas, no dot grid. Nodes are 1px-border
rectangles with mono labels, no filled pills. Edges 1px `#2e333c`; the live path
is a single accent stroke, dashed while running, solid when done. State is one
`StatusDot` language across the whole app: 6px circle plus a word, never a
coloured pill.

**Motion: one grammar.** 120ms ease-out for state, 180ms for overlays, one
accent pulse on a live call. No bounce, no spring overshoot, no entrance
animation on every list item. `MotionConfig reducedMotion="user"` plus a CSS
`prefers-reduced-motion` fallback.

**Anti-generic moves** (because shadcn + Geist + lucide has a recognisable look):
custom button variants (no default shadow, no gradient), a bespoke status strip,
the map above, a rail that reads like a terminal, and mono for all data. A
stranger should not be able to name the component library from the first screen.
The preset is a starting point, not the design: every component in
`src/components/ui/` gets its padding, radius, colour and focus ring replaced by
the values above, and the default shadcn shadows are removed.

Rejected: swapping Geist for a display face to look less like shadcn. The
sameness was weight, scale and spacing, not the family; a second font costs
bundle and looks worse in mono-heavy screens. Rejected also: hand-copying
primitives without the CLI. The spike showed the CLI writes clean Base UI
components in 4 seconds; the work that matters is the styling above, not the
copying.

### Component anatomy — exact, so two engineers draw the same screen

**Button.** Heights 28 (sm) / 32 (md). Padding 10px horizontal (sm), 14px (md).
Radius 8. Border 1px transparent unless secondary. No shadow anywhere.
Text: 12px/16 weight 550 (sm), 13px/18 weight 550 (md); letter-spacing 0.
- primary: fill `accent`, text `#0b0d10`, weight 550. Hover: accent lightened 6%.
  Active: 1px down, no scale.
- secondary: transparent fill, 1px `border strong`, text `text`. Hover: `panel raised`.
- ghost: no border, text `muted`. Hover: `panel raised`, text `text`.
- danger: text `danger`, 1px `danger` at 40% alpha. Hover: danger at 12% alpha fill.
- disabled: 45% opacity, no hover, cursor not-allowed.

**IconButton.** 28×28, radius 8, icon 14px stroke 1.5, tooltip on hover and focus.

**Status strip** (top of Overview, 40px tall, no card): three segments separated
by a 1px `border` rule, left to right: Link (`StatusDot` + mono host or `off`),
Tools (mono count + "tools"), Last call (mono `tool · 84ms` or `—`). Every
segment is `min-width: 0` with `text-overflow: ellipsis` on the mono value and
the full value in a `title`; the strip never wraps and never pushes Revoke off
screen at the enforced 1280px minimum. Revoke sits at the right end as a ghost danger button, visible
only while the link is up.

**StatusDot.** 6px circle plus a 12px label, gap 6, one size everywhere (strip,
rail header, map node). Never a filled pill. ok `#3ecf8e`, warn `#d8a04a`,
danger `#ff6b76` (the same value as danger text), idle `#8f97a5`. Never a filled pill.

**Activity rail.** Fixed 320px column, scrolls internally, max-height = viewport
minus the strip. Each entry: 3px left bar in the status colour; header row 28px
in mono 11px — status, tool, `local` or `link` origin chip, duration, time; body
is the call card (command after `$`, or path with line count); output block mono
12px on `panel raised`, 8px padding, 1px border, `pre` with `max-height: 240px`
and internal scroll, "Show all" only above 20 lines.
Denied (`-32001`), timed-out (`-32002`) and unknown-tool calls appear with their
real status and the hub's message, never hidden.
Empty state: "No calls yet. Connect a chat from the link above." Running entries
first, then finished, newest first. The rail header says `live` and the tooltip
says it clears on app restart; nothing here survives a restart except the
counters in the strip.

**Map.** Canvas `ground`, no dot grid. Node 168×36, radius 8 (nodes are
controls, not panels), 1px `border`, text `text`,
fill `panel`, 12px Geist Mono label, 8px left padding with a 6px StatusDot.
Hub node 200×40, same language, label "hub" plus the port in mono. Edges 1px
`#2e333c`, bezier; live edge 1.5px `accent`, `stroke-dasharray: 6 4` while
running, solid at completion, then settles to `#2e333c`. Those two map beats
(600ms settle, 1.2s fade) are the only exceptions to the 120/180ms grammar.
No minimap, no controls, no background pattern, no node shadows.

**Map accessibility.** Nodes are focusable in reading order (hub, then servers,
then their tools) with a roving tabindex: arrow keys **move focus** between
nodes, `Shift`+arrows pan the canvas, `Enter` opens a details popover and
`Escape` closes it and returns focus to the node, `Home` focuses the hub. A `Map as list` toggle renders the same data as a
plain table (project, tool, status, last call) for screen readers and anyone who
prefers text; it is the same React state, so it updates live, not a screenshot.
A polite live region announces two things only: the link changing state, and an
incoming call (`core_bash via link`), so a screen-reader user learns about a call
without the region reading every result aloud.

**Type ramp (final).** 11 / 12 / 13 / 15 / 19 only. Weights 400 / 550 / 600.
Roles: 11 uppercase tracking 0.06em = section labels and table headers; 12 =
meta, mono values, buttons (sm); 13 = body, buttons (md); 15 = section and card
titles; 19 = page title only. Mono for URLs, durations, tool names, counts,
code, port numbers, paths. Sans for everything else.

**Focus.** Every interactive element uses `:focus-visible` only: 2px `accent`
ring, `outline-offset: 2px`, radius follows the control. No glow, no shadow.

**Replacement map for the current CSS.** `clay-panel` → `Panel` (flat, 1px
`border`, radius 12, no shadow); `clay-bump` → deleted; `Pill` → `StatusDot` +
label or a 6px-radius `Chip`; radii 22/16/999 → 12/8/6; Inter → Geist; the
`360px` rail → 320px; `gap-4`/`gap-3` → 24px between groups, 8px inside one.

### First run — the only path a new user sees

On a fresh profile (no projects, link off, no calls yet) the **Connect a chat**
block takes the rail's grid cell on the right of Overview, so the map keeps its
place and nothing shifts when the intro goes away:

The blast-radius sentence comes **first**: "Anyone with this URL can read,
write and run commands on this computer." It is one line at the top of the
block, in `danger` text, before any button.

1. `Start the link` — the existing action, one button. On failure it shows the
   hub's error inline (not a toast) and the button becomes `Try again`.
2. `Copy the URL` — shows the full `https://…/mcp` in mono with a copy button.
3. `Paste it into ChatGPT, Claude or OpenCode` — one line naming where per
   client, no marketing copy.

It disappears once the link is up and one call has arrived, or when the user
clicks `Hide`; Settings keeps a `Show the intro again` control, so a link that is
up but unused never keeps the intro on the map forever. The strip, not the intro, is what teaches the
state afterwards.

### Trust surface — what the user can always see

- The Link card states the blast radius in one sentence: **"Anyone with this URL
  can read, write and run commands on this computer until you revoke it."**
- Revoke is a two-step inline confirm (same pattern as Remove): first click asks
  `Revoke? This breaks the link permanently.`, second click burns the route.
- Each rail entry shows `local` or `link`, so a shell command is never anonymous.
- After a restart: the rail is empty and says so, the last-call segment reads
  `—`, and the tool count comes from the live list, not from storage. Nothing
  claims a number it cannot prove. The strip shows the link state the app
  actually holds, including `ready (started outside the app)` when a
  systemd-launched tunnel answers but this app did not start it. That state is
  written `ready (external)` everywhere, one string, no variant.
- If the app closes while an approval is pending, the client receives `-32002`
  (unanswered) within its timeout and the rail records it; nothing is left
  hanging silently.

## P1 — Grounded facts

All versions verified 2026-11-26; full table and rejected alternatives in
`docs/UI-STACK.md`.

| decision | verified | beats | why |
|---|---|---|---|
| shadcn CLI | 4.23.1 | hand-rolled | copy-in source, Base UI default |
| Base UI | 1.9.0 | Radix 1.8.1, React Aria 1.22.1 | shadcn default since Jul 2026; `data-*` styling |
| preset | `nova` (style `base-nova`) | vega/maia/lyra | Geist + lucide, closest to our direction |
| Tailwind | 4.3.5 | CSS modules | already present; CSS-first tokens |
| `cn` | 0.5.2 | clsx + tailwind-merge | one compiled dep; fallback isolated |
| motion | 14.14.0 | auto-animate | one reduced-motion switch |
| Geist | 5.3.0 fontsource | `geist` npm 1.9.2 | offline, framework-agnostic |
| Biome | 2.6.9 | ESLint 10 + Prettier | one binary, type-aware |
| Vitest | 5.1.2 | Jest | same Vite pipeline; Node 26.13 |
| store | zustand 5.1.0 | prop drilling, TanStack Query | IPC events push servers/tunnel/approvals/activity; one tiny store |
| Add-MCP form | react-hook-form 7.92.0 + zod 4.6.7 | hand `JSON.parse` | exact JSON-path errors, shadcn form pattern |

## P2 — Steps, each with its check

1. **Dependencies, alias and window floor.** Exact versions (no `^` on
   Base UI/shadcn tooling); `@xyflow/react` moves to `dependencies`; `@types/node`
   added for the config import. Alias is written in **two** places that must
   agree: `tsconfig.json` `paths` (no `baseUrl`, TS 7 removed it) and
   `vite.config.ts` `resolve.alias`. `src-tauri/tauri.conf.json` gets
   `minWidth: 1280`, `minHeight: 720`, so the floor the spec tests is the floor
   the window enforces.
   Check: `bun install` clean, `bun run check` 0, `bun run build` green, and a
   grep shows the alias in both config files; the window refuses to shrink below
   1280×720.
2. **shadcn init.** `bunx shadcn@latest init -y -b base -p nova`; commit before
   and after because it rewrites `src/index.css`; then replace its palette with
   the tokens above, keeping the variable names its components expect.
   Check: `components.json` has `"style": "base-nova"`; `shadcn add button`
   writes a Base UI component; build green.
   Add only: `button dialog badge input textarea select scroll-area tooltip
   skeleton separator card tabs sonner`. Add nothing for calendar, chart, command,
   menubar, carousel or data-table: no screen needs them.
3. **Tokens and type.** `src/index.css` is the only file with colour literals:
   tokens, radii, type scale, focus ring, motion durations, grid variables.
   Check: grep for hex outside `index.css` returns nothing; computed body font
   is Geist; body contrast ≥ 4.5:1 measured from computed styles.
4. **Primitives.** Button (primary/secondary/ghost/danger), IconButton, Input,
   Select, Dialog, Tooltip, StatusDot, Label, KeyValue, EmptyState, ErrorState,
   Skeleton. Each wraps Base UI + `cn`; no bespoke ARIA. **sonner is the only
   toast** — no shadcn Toast primitive is added, so there is never a second
   toast system.
   Check: all of them render on a `?gallery=1` route in the dev server, with a
   visible focus ring reached by Tab and no console errors. Automated axe and
   unit tests come with step 10, so lint green cannot stand in for looks.
5. **Shell and store.** Titlebar keeps the hand-rolled drag (CEF has no Tauri
   drag API); sidebar with Overview, Projects, Settings pinned bottom, logo
   last. A `zustand` store holds servers, tunnel, approvals and activity, fed by
   the Tauri event listeners once at the root.
   Check: `scrollWidth <= clientWidth` at 1280×720; Tab order matches visual
   order; active view has `aria-current="page"`.
5a. **Origin field (the one backend change).** In `src-tauri/src/hub.rs`, add
    `origin: "local" | "link"` to the activity payload and the `tool-start`
    event. Derive it from the **`Host` header compared with the tunnel's
    hostname** — not the peer address, because the tunnel helper forwards to
    `127.0.0.1:8787` and every relayed call would look local. The hub already
    receives the tunnel host for image URLs; reuse it. A request whose `Host`
    matches the tunnel host is `link`; everything else is `local` (including
    all traffic while the tunnel is down). Mirror the field in `src/types.ts`.
    One hub test covers both host values; no other behaviour changes.
    **Limit, stated in the UI copy:** a caller who already holds the link can
    forge `Host`, so origin is best-effort attribution for the human reading the
    rail, not proof. The rail tooltip says exactly that; no security claim rests
    on it.
    Check: `cargo test -p bionichat --lib origin` green; a call to
    `127.0.0.1:8787` with `Host: 127.0.0.1:8787` records `local`, the same call
    with the tunnel host records `link`.
5c. **External helper detection (the second backend change).** After a restart
    the app currently reports `off` even when a helper it did not start still
    holds the public URL. Read the stable hostname from
    `$XDG_DATA_HOME/opentunnel/<profile>/tunnel.json`, probe the helper's lock
    at `127.0.0.1:8789` (the helper binds `port + 2`), and only when both exist
    and `GET https://<host>/mcp` answers, report `ready (external)` with that
    URL. Otherwise report `off`. No new process is started; one unit test covers
    the "lock free" and "hostname missing" paths.
    Check: start the helper by hand, restart the app, the strip says
    `ready (external)`; kill the helper, the strip falls back to `off`.
5b. **First run.** The Connect-a-chat block from P0, shown from a fixture.
    Check: fresh profile renders it; after a call it is gone; Settings can bring
    it back.
6. **Overview, with states.** Status strip (link state, tool count, last call),
   map, live rail. Empty (no servers), loading (first list), error (hub down),
   offline (tunnel down but local hub up), quarantined server.
   Check: each state rendered from a fixture; a real `core_bash` lights hub →
   server → tool and shows running → ok with duration; rail keeps 50 newest and
   marks the call `local` or `link`; Tab reaches every node once, Enter opens the
   details, and the list toggle exposes the same rows.
7. **Projects and Settings.** Folder picker behaviour unchanged. Settings
   grouped Link / Servers / Approvals / Files. The Link group keeps the existing
   controls by name: **Start link when the app opens**, **Open with no window
   next time**, and the last failure reason from `tunnel.log` when the link is
   down — headless users must not lose setup they already rely on. The Link card states the blast
   radius in one sentence and puts **Revoke** beside the URL. Pending approvals
   render in a Dialog (Base UI) with Approve / Deny / Until-restart, plus a
   sonner toast on resolve; the store keeps the 5s poll as a fallback. Add-MCP
   uses react-hook-form + zod so a bad paste shows the exact JSON path. Per-tool
   Allow/Ask/Deny stays; built-ins have no Remove.
   Check: policy change persists to `registry.json`; Deny blocks a call
   (-32001); Ask opens the dialog and Approve lets the call through; Remove asks
   twice (label changes), then removes; Copy yields the full `…/mcp` URL; a bad
   JSON paste shows `line X column Y` and the empty state carries a copyable
   example (`{"mcpServers":{"context7":{"command":"npx","args":["-y","@upstash/context7-mcp"]}}}`); Revoke still burns the route.
8. **Scale and restart truth.** 50 servers, 200-character paths, 10k-character
   output, one very long single line; then restart the app while the tunnel runs
   outside it.
   Check: no horizontal overflow; long values truncate with a title or expand
   control; the rail caps height and scrolls inside itself; after restart the
   rail says it cleared and the strip reads `ready (external)` when step 5c
   detects a helper this app did not start.
9. **Motion.** Route/view transitions, rail enter, map pulse.
   Check: emulate `prefers-reduced-motion: reduce` through CDP
   (`Emulation.setEmulatedMedia`) on the dev server in the in-app browser, then
   repeat against the CEF dev run with `--remote-debugging-port` (dev builds
   accept Chromium switches; release builds do not). Under reduce: computed
   animation-duration ≤ 0.01s and no transform transition. Without it: CLS 0
   during a call.
9b. **Fonts and cleanup.** Self-hosted Geist loaded by CSS; the old Inter /
    system stack removed; `clay-*` classes deleted; `grep -r "clay-" src`
    empty; no static HTML besides `index.html`.
    Check: app renders Geist with the network off.

10. **Hygiene (separate commit).** Biome 2.6.9 config and scripts; Vitest 5 +
    Testing Library + axe-core for the primitive gallery. Moving this after the
    visual pass is deliberate: lint green must not be able to stand in for
    "looks right".
    Check: `bun run lint` 0 errors; `bun run test` green.
11. **CEF proof matrix.** `bun run build` → `touch src-tauri/build.rs` →
    `cargo build -p bionichat` → `scripts/run-cef.sh` on `:0`. Every row below
    is run in CEF, because the in-app browser is a different Chromium:
    - Dialog opens, focus lands on Approve, Tab stays inside, Escape denies and
      focus returns to the invoking button.
    - `cn` merges conflicting classes correctly on one conditional button.
    - Reduced motion via CDP emulation disables the rail and map animation.
    - Tailwind output matches the dev screenshot (hand-check two crops).
    - `bun scripts/conformance.ts http://127.0.0.1:8787/mcp` prints **19/19**;
      the fixtures in `tests/fixtures/` are committed, so a missing one is a
      failure, not a skip.
    Check: all five rows pass, each with the command and its output quoted; at
   the 1280×720 minimum the strip stays on one line with ellipsis, never wraps.
11b. **Docs match the screen.** Update `README.md` (map on Overview, link
    blast radius, no stale demo claims) and regenerate `docs/screenshot.png`
    from the new Overview in the built app.
    Check: no README sentence names a screen that does not exist; the screenshot
    is the new UI, same size budget as before (~90 KB).
11c. **Nothing existing is lost.** Before step 1, copy `registry.json`,
    `settings.json` and `projects.json` aside and record their sha256. After the
    rebuild, compare: no file is rewritten, no policy flips, no project drops.
    The UI may read them differently; it may not change them.
    Check: `sha256sum` before and after are equal for all three files.

12. **Package regression.** `bun run package`, extract clean, run.
    Check: starts, 5 tools listed, one `core_bash` returns, closes.

### Risks

| risk | likelihood | mitigation |
|---|---|---|
| Generic shadcn look survives the tokens | medium | anti-generic moves above are acceptance criteria, judged on a screenshot before merge |
| shadcn init overwrites `index.css` badly | high (it does rewrite it) | commit first; port tokens deliberately, keep its variable names |
| Base UI + TS 7 friction | retired | spike passed |
| Scope creep into features | high | non-goals; anything else becomes a note |
| Bundle growth | low | baseline measured 2026-11-26: 502,503 B js + 43,406 B css; compare at steps 4 and 12 |

## Definition of done

1. Every check above passes, with the command and output quoted.
2. The binary from this tree, on `:0`, shows the new UI and performs a real call.
3. `bun run check`, `bun run build`, lint, tests, the five CEF matrix rows and
   conformance 19/19 green.
4. A stranger can say what the app does from the first screen alone.
5. Three screenshots (first run, idle, mid-call) reviewed against the
   anti-generic list, plus one of the approval dialog with a visible focus ring.
6. `dist` size change stated as a number.

## Not tested by this spec

macOS rendering, Windows binary, installers.
