import type { ReactNode } from "react";

export function Panel({
  title,
  right,
  children,
  labelledBy,
  className = "",
}: {
  title?: ReactNode;
  right?: ReactNode;
  children: ReactNode;
  labelledBy?: string;
  className?: string;
}) {
  return (
    <section
      aria-labelledby={labelledBy}
      className={`clay-panel px-5 pt-4 pb-5 ${className}`}
    >
      {title !== undefined && (
        <div className="mb-3 flex items-baseline justify-between gap-3">
          <h2
            id={labelledBy}
            className="text-[13.5px] font-[550] tracking-[-0.005em]"
          >
            {title}
          </h2>
          {right}
        </div>
      )}
      {children}
    </section>
  );
}

export function Button({
  primary,
  small,
  disabled,
  onClick,
  children,
  title,
}: {
  primary?: boolean;
  small?: boolean;
  disabled?: boolean;
  onClick?: () => void;
  children: ReactNode;
  title?: string;
}) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={[
        "clay-button",
        primary ? "clay-button-primary" : "",
        small ? "clay-button-sm" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      {children}
    </button>
  );
}

/** A label above its value, both on one raised bump. */
export function Stat({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-[11.5px] text-dim">{label}</span>
      <span className="font-mono text-[17px] leading-none tabular-nums">{value}</span>
    </div>
  );
}
