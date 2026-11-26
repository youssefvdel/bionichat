# Spec — bionichat UI rebuild

Status: **draft, round 0**
Date: 2026-11-26
Owner: this session; reviewed by fresh-context reviewers (P3)

## P0 — Problem

The dashboard works and looks generated. That is costly for a product whose
whole pitch is "your own PC, from any chat": a stranger judges the tool by the
first screen, and the current screen reads as an internal prototype. The user's
words: "the ui looks ugly". Three restyle attempts (green, warm paper,
claymorphism) were rejected, so the problem is structural, not a colour swap.

Users: Youssef first, then anyone who runs the app and connects a web chat.

Goal: an interface that looks intentionally designed — dense, quiet, precise —
with every state legible at a glance, and no regressions in function.

### Non-goals

- No chat UI. Removed earlier on purpose.
- No new backend features, no tool behaviour changes.
- No installer work, no macOS, no Windows rebuild in this spec.
- No theming system for end users; one theme, done well.
- No marketing page.

### Constraints

- One renderer: CEF (Chromium 156) on Linux and Windows, wry/WebKitGTK as the
  macOS CI path. No CSS that only works in Chrome and not WebKitGTK.
- The frontend is embedded in the binary at compile time; every build that
  changes `src/` must touch `src-tauri/build.rs`.
- 1280×720 is the floor for the window; the layout must not scroll the body
  horizontally at that size.
- Offline: no network fonts, no CDN.
- Keyboard reachable, visible focus, reduced motion honoured.

### Assumptions and their cost if wrong

| assumption | cost if wrong |
|---|---|
| Base UI 1.9 + React 19.3 + TS 7 typecheck together | component layer rewritten or pinned back to Radix |
| `shadcn add` works with a hand-made `components.json` in this repo | copy component source manually (a day) |
| Motion 14 respects `prefers-reduced-motion` inside CEF | hand-rolled CSS transitions instead |
| The current Tauri commands cover every view state | small backend additions, out of scope here |
| Tailwind 4.3 renders the same in WebKitGTK | macOS-only CSS fixes later |

### Kill criteria

- If Base UI cannot typecheck with TS 7 after one focused attempt, stop and
  keep the current UI for 0.2.0 rather than ship a half-migrated tree.
- If the rebuild cannot be proven in CEF from the built binary, it is not done
  and does not merge.

## P1 — Grounded facts

All versions verified 2026-11-26 from the live registry; full table with the
rejected alternatives in `docs/UI-STACK.md`.

| decision | verified | beats | why |
|---|---|---|---|
| shadcn CLI | 4.23.1 | hand-rolled components | copy-in source, current defaults, Base UI |
| Base UI | 1.9.0 | Radix 1.8.1, React Aria 1.22.1 | shadcn default since Jul 2026; `data-*` styling |
| Tailwind | 4.3.5 | CSS modules, vanilla-extract | already present, CSS-first tokens |
| `cn` | 0.5.2 | clsx + tailwind-merge | one compiled dependency; fallback kept |
| motion | 14.14.0 | auto-animate, CSS-only | one reduced-motion switch for the app |
| Geist | 5.3.0 (fontsource) | `geist` npm 1.9.2 | offline, framework-agnostic |
| Biome | 2.6.9 | ESLint 10 + Prettier | one binary, type-aware domains |
| Vitest | 5.1.2 | Jest | same Vite pipeline; Node 26.13 ok |
| axe-core | 4.14.1 | vitest-axe 0.1.0 | maintained; drives a11y checks |

Unknowns carried into P2: see "must be proven" in `docs/UI-STACK.md`.

## P2 — Steps, each with its check

1. **Move `@xyflow/react` to dependencies; add the stack.** Install shadcn,
   Base UI, `cn`, cva, lucide, tw-animate-css, motion, sonner, fonts, Biome,
   Vitest, Testing Library, axe-core, with **exact versions** (no `^` on Base UI
   or shadcn tooling).
   Check: `bun install` clean, `bun run check` 0 errors, `bun run build` green.
2. **Wire the alias and shadcn.** `@/*` in `tsconfig.json` paths and
   `vite.config.ts` resolve.alias; hand-write `components.json` (Base UI,
   `new-york` style family, our paths); CSS imports `tailwindcss`,
   `tw-animate-css`, `shadcn/tailwind.css`.
   Check: `bunx shadcn@latest add button` writes `src/components/ui/button.tsx`
   and the app still builds.
3. **Tokens and type.** Rewrite `src/index.css` as the single source: colour,
   radius, spacing rhythm, type scale (Geist/Geist Mono), focus ring, motion
   durations. Dark only.
   Check: computed `font-family` in the dev page is Geist; no hex literals
   outside `index.css` (grep); contrast ≥ 4.5:1 for body text and ≥ 3:1 for
   borders that carry meaning, measured from computed styles.
4. **Primitives.** Button, IconButton, Field/Input, Select, Dialog, Tooltip,
   Toast, StatusDot, KeyValue row. Each wraps Base UI + `cn`; no bespoke ARIA.
   Check: component tests render each; axe-core reports no violations.
5. **Shell.** Titlebar (drag region stays hand-rolled — the CEF runtime does not
   implement Tauri's drag API), sidebar (Overview / Projects, Settings pinned
   bottom, logo last), content grid at 1280×720.
   Check: `document.documentElement.scrollWidth <= clientWidth`; sidebar items
   reachable by Tab in visual order; active item announced via `aria-current`.
6. **Overview.** Status strip (link state, counts), live tool map, activity rail
   with terminal-style call cards.
   Check: a real `core_bash` call lights hub → server → tool, shows running then
   ok with duration; rail keeps 50 newest.
7. **Projects and Settings.** Folder picker unchanged in behaviour, restyled;
   Settings grouped into Link / Servers / Approvals / Files with per-tool
   Allow-Ask-Deny and Remove (not for built-ins).
   Check: policy change persists to `registry.json`; Remove asks twice; Copy
   writes the full `…/mcp` URL.
8. **Motion pass.** Route and view transitions, rail enter, map pulse; all via
   `MotionConfig reducedMotion="user"` plus CSS `@media (prefers-reduced-motion)`.
   Check: with reduced motion emulated, no transform/opacity animation runs
   (`getComputedStyle` duration ≤ 0.01s, or motion disabled).
9. **CEF build and live proof.** `bun run build` → touch `build.rs` →
   `cargo build -p bionichat` → run on `:0`.
   Check: all views render, a real tool call works, conformance 19/19 over the
   local hub.
10. **Package regression.** `bun run package`, extract to a clean directory, run
    the binary.
    Check: it starts, hub answers `tools/list` with 5 tools, one `core_bash`
    call returns, then it closes.

### Risks

| risk | likelihood | mitigation |
|---|---|---|
| Base UI + TS 7 type friction | medium | prove with `button` + `dialog` before adding more |
| shadcn CLI rewrites `index.css` | medium | manual path; commit before running any `add` |
| CSS regression in WebKitGTK (macOS) | low | avoid `:has()`-only layouts, test in CI later |
| Scope creep into features | high | non-goals above; anything else becomes a note |
| Bundle growth | low | measure `dist` per step |

## Definition of done

1. Every check above passes, with the command and output quoted in the report.
2. The binary built from this tree, run on `:0`, shows the new UI and performs a
   real tool call.
3. `bun run check`, Biome, unit tests and conformance are green.
4. A stranger can tell what the app does from the first screen without help.
5. `dist` size change is stated as a number.

## Not tested by this spec

- macOS rendering.
- Windows binary.
- Installers.
