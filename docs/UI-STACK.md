# UI stack study — bionichat, 26 Nov 2026

Every version below was read from the live registry on **2026-11-26** (npm
`/<pkg>` latest, crates.io API). Docs were read at the same time: shadcn
`installation/vite.mdx` and `installation/manual.mdx` on `main`, the Base UI
1.9 docs, Tailwind v4 docs, the Vitest 5 migration guide.

The question this answers: which tools give a bespoke, accessible, fast desktop
UI that survives CEF (Chromium 154) and does not fight the existing Vite build.

## What already exists and stays

| piece | version | why it stays |
|---|---|---|
| React | 19.3.1 | current stable, matches `@types/react` 19.3 |
| Vite | 8.3.8 | current, `es2022` target, one build feeding Tauri |
| `@vitejs/plugin-react` | 6.3.0 | current, React 19 support |
| TypeScript | 7.0.2 | native compiler; `tsc --noEmit` already clean on this tree |
| Tailwind | 4.3.5 + `@tailwindcss/vite` 4.3.5 | CSS-first config, no `tailwind.config.js` |
| `@xyflow/react` | 13.0.3 | the live tool map; no replacement needed |

`@xyflow/react` sits in `devDependencies` today. It is a runtime import, so it
moves to `dependencies`.

## Decisions

### Components: shadcn/ui on Base UI, copied into the repo

- `shadcn` CLI **4.23.1** (published 2026-11-25), Base UI primitives
  `@base-ui/react` **1.9.0** (2026-10-09).
- shadcn made **Base UI the default in July 2026**; Radix (`radix-ui` 1.8.1) is
  still supported but is no longer where new components land first.
- Base UI exposes state as `data-*` attributes and a `render` prop instead of
  Radix's `asChild`, so Tailwind styles states directly
  (`data-starting-style:opacity-0`, `data-popup-open:`), no CSS-in-JS.
- Components are copied into `src/components/ui/`, so we own them and can cut
  anything that does not fit. That is the point: no library look.

Rejected: **Radix 1.8.1** (fine, not first-class for new shadcn components);
**React Aria Components 1.22.1** (best a11y/i18n, but heavier API and no shadcn
default; we have no date or locale-heavy UI); **Ark UI 5.40.1** (good, smaller
ecosystem, no shadcn support); **MUI / Mantine / HeroUI** (their look arrives
with the components, and the goal is a bespoke one).

### Class merging: `cn` 0.5.2, with a fallback

`cn` (shadcn, 2026-11-02) is a compiled drop-in for `clsx` + `tailwind-merge`.
One dependency instead of two, and it is what the current shadcn components
import.

Risk: it is young (0.5.2). Fallback is `clsx` 2.1.1 + `tailwind-merge` 3.7.1,
which are mature and API-compatible. The shadcn installer writes
`src/lib/utils.ts` exporting `cn`; that file is the only place that decides the
implementation, so the fallback is a one-file swap.

### Icons: `lucide-react` 1.81.0

shadcn's default icon set, published the same day (2026-11-26), tree-shaken per
icon. Rejected `@tabler/icons-react` 3.53.0 (larger set, different weight
language) and `@phosphor-icons/react` 2.1.10 (last publish 2025-05-22).

### Type: Geist Sans + Geist Mono, self-hosted

`@fontsource-variable/geist` and `@fontsource-variable/geist-mono` **5.3.0**.
Self-hosted because the app ships offline and must not fetch fonts; variable
because one file covers all weights. The `geist` npm package 1.9.2 is Next-only,
so it loses here.

### Motion: `motion` 14.14.0, one config for reduced motion

`import { motion } from "motion/react"`. `<MotionConfig reducedMotion="user">`
at the root makes every animation honour `prefers-reduced-motion`.
Used for: view transitions, the activity rail entering, the map path pulse.
Rejected `@formkit/auto-animate` 0.11.0 (no control over choreography) and
`framer-motion` (renamed to `motion`; importing the old package would ship the
same code under a deprecated name).

### Feedback: `sonner` 2.0.8

Toasts for copy-link, policy saved, tunnel errors. One component, accessible,
themeable with CSS variables. Rejected building a toast system by hand.

### Lint and format: Biome 2.6.9

One binary for lint + format, type-aware rules through `linter.domains.types`
(`noFloatingPromises`, `noMisusedPromises`), and it replaces ESLint + Prettier.
Rejected **ESLint 10.15.0** (two tools, slower, more config) and **oxlint
1.95.0** (fast, but formatting still needs another tool).

### Tests: Vitest 5.1.2 + Testing Library

- `vitest` **5.1.2**, `@testing-library/react` **16.3.3**,
  `@testing-library/jest-dom` **8.0.0**, `jsdom` **31.0.1**.
- Vitest 5 requires Vite ≥ 6.4 and Node ≥ 22.12; this box has Vite 8.3.8 and
  Node 26.13.0, so the requirement is met.
- `clearMocks` defaults to `true` in v5, and `vite` is a peer dependency now.
- Accessibility: `axe-core` **4.14.1** run against rendered views. `vitest-axe`
  is stale (0.1.0, 2022) and loses.

Rejected **Playwright 1.65.1** for now: it cannot attach to the shipped CEF app
without a remote-debugging port that release builds refuse to open. DOM proof
runs against `vite dev` in the in-app browser instead; Playwright returns when
there is a web build worth testing end to end.

### State and forms: added after the second review

- **zustand 5.1.0** for servers, tunnel, approvals and activity. The Tauri event
  stream is push-based and three views read the same state, so prop drilling is
  the current pain. Rejected react-query 5.108.4 (REST caching model, wrong
  shape for IPC events) and jotai 3.0.2 (atom-per-field is more ceremony here).
- **react-hook-form 7.92.0 + zod 4.6.7** for the Add-MCP JSON box only. Zod
  reports the exact JSON path of a bad paste; the Rust side already validates
  again before saving. Used nowhere else.

### Skipped on purpose
- **Tables/charts** (react-table 9.3.1, recharts 3.11.1): nothing to tabulate or
  chart.
- **Playwright**: see above; the dev server plus the in-app browser is the DOM
  oracle for this pass.
- **Markdown/Shiki**: no chat, no rendered output.

## Integration facts taken from the docs, not memory

1. Tailwind v4 installs as a Vite plugin and a single
   `@import "tailwindcss";`; tokens go in `@theme`. No config file.
2. shadcn in an existing Vite project needs: dependencies, an import alias
   (`tsconfig.json` `paths` **and** `vite.config.ts` `resolve.alias`), a
   `components.json`, and CSS that imports `tailwindcss`, `tw-animate-css` and
   `shadcn/tailwind.css`.
3. `shadcn init` on an existing Vite project validates the alias and rewrites
   `src/index.css`; the spike ran it and it worked. It **is** the path used, with
   a commit before and after so the rewrite is visible, and the tokens are then
   ported deliberately instead of keeping its palette.
4. Base UI styles through `data-*` state attributes and the `render` prop, so any
   Tailwind build works and no extra styling runtime is needed.
5. Vitest 5 needs an explicit `vite` peer (Bun installs peers automatically, so
   this is satisfied here).

## Proven in the spike, 2026-11-26 (`/tmp/opencode/ui-spike`)

- `shadcn init -b base -p nova` on an existing Vite + Tailwind v4 project:
  detected Vite, validated the alias, wrote `components.json` with
  `"style": "base-nova"`, installed `@base-ui/react ^1.9.0`, `cn ^0.5.2`,
  Geist, lucide and tw-animate-css.
- `shadcn add button dialog select tooltip sonner badge separator`: 7 files,
  no errors.
- A page using Button, Dialog, Select, Tooltip and sonner: `tsc --noEmit` exit
  0, `vite build` green. Spike bundle 437 kB js / 39 kB css, fonts emitted.
- **TypeScript 7 removed `baseUrl`** (TS5102): the alias must be `paths` only.

## What must still be proven during implementation

- `cn` 0.5.2 behaves like `clsx`+`tailwind-merge` on conditional classes; swap
  to the fallback if any class merge is wrong.
- Motion respects reduced motion inside CEF, not only in Chrome.
- Tailwind 4.3 output looks identical in Chromium **154** (this app's CEF,
  `cef 154.5.0+154.0.34`) and the dev browser.
- Base UI Dialog focus trap inside CEF.
- Bundle cost: measure `dist` before and after. A 20% jump needs a reason.
