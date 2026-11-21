import type { ReactNode } from "react";

export function Card({
  title,
  right,
  children,
  labelledBy,
}: {
  title: ReactNode;
  right?: ReactNode;
  children: ReactNode;
  labelledBy: string;
}) {
  return (
    <section
      aria-labelledby={labelledBy}
      className="clay-panel px-5 pt-4 pb-5"
    >
      <div className="mb-3 flex items-baseline justify-between gap-3">
        <h2 id={labelledBy} className="text-[14px] font-[550] tracking-[-0.005em]">
          {title}
        </h2>
        {right}
      </div>
      {children}
    </section>
  );
}

export function ActionButton({
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
