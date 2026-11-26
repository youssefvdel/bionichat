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

- No chat UI, no new backend features, no tool behaviour changes.
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
| faint | `#6b7280` |
| accent | `#4c9dff` (the only saturated colour that is not a status) |
| ok | `#3ecf8e` |
| warn | `#d8a04a` |
| danger | `#e0555f` |

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

1. **Dependencies and alias.** Exact versions (no `^` on Base UI/shadcn tooling);
   `@xyflow/react` moves to `dependencies`; `paths` alias only (no `baseUrl`).
   Check: `bun install` clean, `bun run check` 0, `bun run build` green.
2. **shadcn init.** `bunx shadcn@latest init -y -b base -p nova`; commit before
   and after because it rewrites `src/index.css`; then replace its palette with
   the tokens above, keeping the variable names its components expect.
   Check: `components.json` has `"style": "base-nova"`; `shadcn add button`
   writes a Base UI component; build green.
   Add only: `button dialog badge input textarea select scroll-area tooltip
   skeleton separator card tabs`. Add nothing for calendar, chart, command,
   menubar, carousel or data-table: no screen needs them.
3. **Tokens and type.** `src/index.css` is the only file with colour literals:
   tokens, radii, type scale, focus ring, motion durations, grid variables.
   Check: grep for hex outside `index.css` returns nothing; computed body font
   is Geist; body contrast ≥ 4.5:1 measured from computed styles.
4. **Primitives.** Button (primary/secondary/ghost/danger), IconButton, Input,
   Select, Dialog, Tooltip, Toast, StatusDot, Label, KeyValue, EmptyState,
   ErrorState, Skeleton. Each wraps Base UI + `cn`; no bespoke ARIA.
   Check: each renders in a test; axe-core 0 violations on a gallery page.
5. **Shell and store.** Titlebar keeps the hand-rolled drag (CEF has no Tauri
   drag API); sidebar with Overview, Projects, Settings pinned bottom, logo
   last. A `zustand` store holds servers, tunnel, approvals and activity, fed by
   the Tauri event listeners once at the root.
   Check: `scrollWidth <= clientWidth` at 1280×720; Tab order matches visual
   order; active view has `aria-current="page"`.
6. **Overview, with states.** Status strip (link state, tool count, last call),
   map, live rail. Empty (no servers), loading (first list), error (hub down),
   offline (tunnel down but local hub up), quarantined server.
   Check: each state rendered from a fixture; a real `core_bash` lights hub →
   server → tool and shows running → ok with duration; rail keeps 50 newest.
7. **Projects and Settings.** Folder picker behaviour unchanged. Settings
   grouped Link / Servers / Approvals / Files. The Link card states the blast
   radius in one sentence and puts **Revoke** beside the URL. Pending approvals
   render in a Dialog (Base UI) with Approve / Deny / Until-restart, plus a
   sonner toast on resolve; the store keeps the 5s poll as a fallback. Add-MCP
   uses react-hook-form + zod so a bad paste shows the exact JSON path. Per-tool
   Allow/Ask/Deny stays; built-ins have no Remove.
   Check: policy change persists to `registry.json`; Deny blocks a call
   (-32001); Ask opens the dialog and Approve lets the call through; Remove asks
   twice (label changes), then removes; Copy yields the full `…/mcp` URL; a bad
   JSON paste shows `line X column Y`; Revoke still burns the route.
8. **Scale rules.** 50 servers, 200-character paths, 10k-character tool output,
   one very long single line.
   Check: no horizontal overflow; long values truncate with a title or expand
   control; the rail caps height and scrolls inside itself.
9. **Motion.** Route/view transitions, rail enter, map pulse.
   Check: with reduced motion emulated, computed animation duration ≤ 0.01s and
   no transform transition runs; without it, no layout shift (CLS measured 0
   on a call).
9b. **Fonts and cleanup.** Self-hosted Geist loaded by CSS; the old Inter /
    system stack removed; `clay-*` classes deleted; `grep -r "clay-" src`
    empty; no static HTML besides `index.html`.
    Check: app renders Geist with the network off.

10. **Hygiene (separate commit).** Biome 2.6.9 config and scripts; Vitest 5 +
    Testing Library + axe-core for the primitive gallery. Moving this after the
    visual pass is deliberate: lint green must not be able to stand in for
    "looks right".
    Check: `bun run lint` 0 errors; `bun run test` green.
11. **CEF proof.** `bun run build` → touch `build.rs` → `cargo build -p bionichat`
    → run on `:0`.
    Check: all views render; a real tool call works; conformance 19/19.
12. **Package regression.** `bun run package`, extract clean, run.
    Check: starts, 5 tools listed, one `core_bash` returns, closes.

### Risks

| risk | likelihood | mitigation |
|---|---|---|
| Generic shadcn look survives the tokens | medium | anti-generic moves above are acceptance criteria, judged on a screenshot before merge |
| shadcn init overwrites `index.css` badly | high (it does rewrite it) | commit first; port tokens deliberately, keep its variable names |
| Base UI + TS 7 friction | retired | spike passed |
| Scope creep into features | high | non-goals; anything else becomes a note |
| Bundle growth | low | measure `dist` at steps 4 and 12 |

## Definition of done

1. Every check above passes, with the command and output quoted.
2. The binary from this tree, on `:0`, shows the new UI and performs a real call.
3. `bun run check`, `bun run build`, lint, tests and conformance green.
4. A stranger can say what the app does from the first screen alone.
5. Two screenshots (idle, mid-call) reviewed against the anti-generic list.
6. `dist` size change stated as a number.

## Not tested by this spec

macOS rendering, Windows binary, installers.
