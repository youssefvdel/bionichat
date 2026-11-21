export function Mark({ size = 18 }: { size?: number }) {
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

const pillFor: Record<string, string> = {
  pending: "bg-raised text-warn",
  ready: "bg-raised text-good",
  error: "bg-raised text-bad",
  starting: "bg-raised text-dim",
  off: "bg-raised text-dim",
  idle: "bg-raised text-dim",
  ok: "bg-raised text-good",
  denied: "bg-raised text-bad",
  timeout: "bg-raised text-dim",
};

export function Pill({ status, label }: { status: string; label?: string }) {
  const cls = pillFor[status] ?? pillFor["idle"];
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[11px] leading-none whitespace-nowrap tabular-nums ${cls}`}
    >
      <span className="inline-block h-1.5 w-1.5 rounded-full bg-current" />
      {label ?? status}
    </span>
  );
}
