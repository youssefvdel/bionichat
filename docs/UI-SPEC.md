# bionichat — UI rebuild spec (planning phase, 2026-11-26)

> Fresh-start plan. Nothing in `src/` is sacred except behaviour.
> Behaviour to keep: Overview / Projects / Settings, tunnel status,
> approvals gate, servers+tools list, live map, activity feed, custom titlebar.

## P0 — Problem, non-goals, success

**Problem:** current UI looks generated. Hand-rolled `ui.tsx` + `chrome.tsx`
+ 368 lines of `clay-*` CSS drift: every screen invents its own panel,
button, focus ring. No token system, no headless primitives, icons are
inline SVG, motion is one keyframe.

**Goal:** a dense dark dashboard that looks designed: one accent, 8px grid,
real focus rings, motion only where it explains state.

**Non-goals:**
- No new backend, no new MCP protocol, no chats in this pass.
- No logo work. One file stays: `assets/logo/bionichat-master.svg`.
  17 concept/render files already deleted. No new logo files.
- No static HTML screens. Only entry `index.html` + React.
- No installers in this pass (separate lane, needs `resource_dir()` fix).

**Success checks (run, not eyeball):**
1. `bun run check` clean (tsc --noEmit, TS 7 native).
2. `bun run build` clean, then `cargo build -p bionichat` paints dashboard on CEF via `scripts/run-cef.sh`.
3. Approvals flow: Ask tool → banner/dialog → Approve/Deny → hub answers; Revoke still burns route.
4. Tunnel button → public URL → serves `/mcp` tools/list (existing conformance still 19/19).
5. Keyboard: Tab reaches every button/dialog, focus ring visible, `prefers-reduced-motion` kills motion.
6. DOM proof: `getBoundingClientRect` spacing on 8px grid, `scrollWidth <= clientWidth` (no overflow), one accent computed value.

**Kill criteria:** if shadcn Base UI breaks under CEF (dialog portal / focus trap fails), stop and fall back to current `ui.tsx` + tokens only. No Radix detour.

**Assumptions ledger:**
- Tauri v3 alpha.6 + `tauri-runtime-cef` alpha.7 stay pinned (verified in `src-tauri/Cargo.toml` today). Cost if wrong: full shell rebuild.
- CEF == Chromium 154: modern CSS OK, avoid heavy `backdrop-filter` over large areas (perf). Cost if wrong: jank on map.
- shadcn CLI works against existing Vite project via `init` + `add` (docs verified today). Cost if wrong: hand-wire tokens.

## P1 — Decisions (verified live 2026-11-26 via `npm view` + official docs)

| job | choice | verified | why it wins | loser + why | source |
|---|---|---|---|---|---|
| framework | React 19.3.1 + Vite 8.3.8 + `@vitejs/plugin-react` 6.3.0 | npm today | already in tree, React 19 actions/Suspense fit event UI | — | npm `react`, `vite` |
| types | TypeScript 7.0.2 native compiler | npm + `tsc --noEmit` clean | 10x faster, no config change | TS 5.x tsc slow path | npm `typescript` |
| styling | Tailwind 4.3.5 CSS-first `@theme`, no config file | npm today | current docs path, tokens in CSS | v3 config file + `tailwind.config.js` (legacy) | https://ui.shadcn.com/docs/installation/vite, npm `tailwindcss` |
| components | shadcn CLI 4.23.1, **Base UI** primitives, preset `nova` | npm `shadcn@4.23.1` | Base UI is the shadcn default since July 2026; unstyled + themeable | Radix (slower path now, still supported); MUI/Mantine (whole-library look, not tokens); hand-rolled (current drift) | https://ui.shadcn.com/docs/installation/vite, https://base-ui.com/react/overview/quick-start |
| primitives pkg | `@base-ui/react` 1.9.0 | npm today | ships with shadcn Base components | `@radix-ui/*` per-piece install | npm `@base-ui/react` |
| motion | `motion` 14.14.0 via `motion/react` | npm today | WAAPI + springs, `useReducedMotion` + `AnimatePresence` for approvals/map | framer-motion import path (legacy name); pure CSS (can't do interruptible/layout) | https://motion.dev/docs/react |
| map | `@xyflow/react` 13.0.3 stays | npm today | already wired in `ToolFlow.tsx`, dark theme via CSS vars | rewire to new lib = risk, no gain | npm `@xyflow/react` |
| icons | `lucide-react` 1.81.0 | npm today | one stroke system, shadcn default | inline SVG in `chrome.tsx`/`Titlebar.tsx` (drift, no set) | npm `lucide-react` |
| fonts | `@fontsource-variable/geist` + `geist-mono` 5.3.0 self-hosted | npm today | Geist matches `nova`, no network at runtime (CEF offline-safe) | Google Fonts link (fails offline, FOUT in app) | npm `@fontsource-variable/*` |
| store | `zustand` 5.1.0 | npm today | IPC events are push-based; tiny store for servers/tunnel/approvals/activity | TanStack Query (built for REST caching, overkill here; keep polling fallback in store) | npm `zustand` vs `5.108.4` query |
| forms | `react-hook-form` 7.92.0 + `zod` 4.6.7 | npm today | Add-MCP paste-JSON validation with errors, shadcn form pattern | hand `JSON.parse` + string error (current, no schema) | npm `react-hook-form`, `zod` |
| toasts | `sonner` 2.0.8 | npm today | one line for approval/activity toasts, dark theme prop | shadcn toast alone (more wiring for same result) | npm `sonner` |
| alias | `@/*` → `./src/*` in `tsconfig.json` + `vite.config.ts` | shadcn docs | required by every shadcn import | relative `../components` sprawl | https://ui.shadcn.com/docs/installation/vite |

**Install (exact, after spec review):**
```bash
bun add @base-ui/react@1.9.0 motion@14.14.0 lucide-react@1.81.0 zustand@5.1.0 sonner@2.0.8 react-hook-form@7.92.0 zod@4.6.7 @fontsource-variable/geist@5.3.0 @fontsource-variable/geist-mono@5.3.0
bun add -d @types/node
# then: bunx shadcn@4.23.1 init (preset nova, Base UI) + bunx shadcn@4.23.1 add button dialog badge input select scroll-area tooltip skeleton separator card tabs
```

**What to add from shadcn (and what NOT):**
- Add: `button` (all actions), `dialog`/`alert-dialog` (approvals gate), `badge` (status dots), `input`/`textarea`/`select` (Add-MCP, settings), `scroll-area` (activity), `tooltip` (icon buttons), `sonner` wiring, `skeleton` (servers null state), `separator`, `card` (replaces `clay-panel`), `tabs` if Settings grows.
- Do NOT add: `calendar`, `carousel`, `chart`, `data-table`, `command`, `menubar`, date-picker — no screen needs them; each is dead weight in CEF bundle.

## P2 — Build order (each step proves before next)

1. **Tokens + alias, no components yet.** `@/*` alias in `tsconfig.json` + `vite.config.ts`; `@theme` tokens (ground/ink/accent/good/bad/warn, 8px radius scale, Geist vars); delete `clay-*` only after shadcn lands. Check: `bun run check` + visual diff of one Panel.
2. **shadcn init + base set.** `init` then add the list above. Migrate `ui.tsx` Panel/Button/Stat → `card`/`button`/`badge`. Check: Overview renders, keyboard focus ring visible.
3. **Titlebar + Sidebar.** Lucide icons replace inline SVG; keep manual `window_move` drag (CEF has no native drag region — see `Titlebar.tsx:64`). Keep `Mark` from single master SVG. Check: min/max/close via commands, double-click toggles maximize.
4. **Approvals → dialog + sonner.** `dialog` for pending approvals, `sonner` toast on allow/deny/timeout. Keep 5s poll fallback in store. Check: ask→dialog→approve→hub call succeeds.
5. **Servers + Add-MCP form.** RHF+zod JSON validation, exact error line, red status row. Check: bad JSON shows line, app stays up.
6. **Live map polish.** Keep `@xyflow/react`, theme via CSS vars to tokens, `motion` only for running-path pulse; `useReducedMotion` disables. Check: 50-node map pans at 60fps, reduced-motion = static.
7. **Activity + LiveRail.** `scroll-area` + `skeleton` loading, `motion/layout` for row-in (replaces `row-in` keyframe). Check: no horizontal overflow (`scrollWidth<=clientWidth`).
8. **Fonts + cleanup.** Self-hosted Geist, remove Inter/system stack, remove remaining `clay-*` CSS, confirm no `*.html` besides entry. Check: offline launch renders Geist, `grep -r "clay-" src` empty.

**Rules for the pass:**
- Dark, dense, one accent (`#4c9dff` stays unless tokens review says otherwise). Real `:focus-visible` rings. 8px grid.
- Motion only explains state (approval in, running pulse, toast in/out). Honour reduced-motion.
- DOM measurements are proof, screenshots are not.
- One build per tree: never Vite + cargo together (phantom `dist/<hash>` error). Order: `bun run build && touch src-tauri/build.rs && cargo build -p bionichat`.

## P3 — Review gate (before code)

Run fresh-eyes review on THIS file only (goldfish: no conversation, spec + sources). At least: design critic, implementation readiness, zoom-out. Two clean rounds (no Blocker/Major) → freeze → build. Rejections recorded with reason.

## P4 — Freeze

Spec freezes as committed file + round count. User approves install step (new deps, irreversible-ish). Plan drift = update this file in same change.
