/* The product mark, its status chips, and the three nav icons.
   One stroke weight, one drawing style. */

const tone: Record<string, string> = {
  pending: "bg-warn/15 text-warn",
  ready: "bg-good/15 text-good",
  error: "bg-bad/15 text-bad",
  starting: "bg-bump text-sub",
  off: "bg-bump text-dim",
  idle: "bg-bump text-dim",
  ok: "bg-good/15 text-good",
  denied: "bg-bad/15 text-bad",
  timeout: "bg-bump text-dim",
};

export function Mark({ size = 17 }: { size?: number }) {
  return (
    <svg
      viewBox="0 0 256 256"
      width={size}
      height={size}
      fill="currentColor"
      aria-hidden="true"
      className="block"
    >
      <path d="M 133 45 L 65 45 L 45 65 L 45 133 L 51.5 139.5 L 72 119 L 72 75 L 75 72 L 119 72 L 139.5 51.5 Z" />
      <path d="M 197.5 45 L 160 45 L 45 160 L 45 197.5 Z" />
      <path d="M 58.5 211 L 96 211 L 211 96 L 211 58.5 Z" />
      <path d="M 123 211 L 191 211 L 211 191 L 211 123 L 204.5 116.5 L 184 137 L 184 181 L 181 184 L 137 184 L 116.5 204.5 Z" />
    </svg>
  );
}

export function Pill({ status, label }: { status: string; label?: string }) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-[5px] text-[11.5px] leading-none font-[520] whitespace-nowrap tabular-nums ${
        tone[status] ?? tone["idle"]
      }`}
    >
      <span className="inline-block h-1.5 w-1.5 rounded-full bg-current" />
      {label ?? status}
    </span>
  );
}

function Icon({ children }: { children: React.ReactNode }) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className="shrink-0"
    >
      {children}
    </svg>
  );
}

export function OverviewIcon() {
  return (
    <Icon>
      <circle cx="8" cy="8" r="5.6" />
      <circle cx="8" cy="8" r="1.6" fill="currentColor" stroke="none" />
    </Icon>
  );
}

export function ProjectsIcon() {
  return (
    <Icon>
      <rect x="2.4" y="2.6" width="8" height="8" rx="2" />
      <path d="M5.6 13.4h6a2 2 0 0 0 2-2v-6" />
    </Icon>
  );
}

export function SettingsIcon() {
  return (
    <Icon>
      <path d="M2.6 5h4M9.4 5h4M2.6 11h6M11.4 11h2" />
      <circle cx="7.6" cy="5" r="1.9" />
      <circle cx="9.6" cy="11" r="1.9" />
    </Icon>
  );
}
