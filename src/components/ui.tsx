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
    <section aria-labelledby={labelledBy} className="rounded-md border border-edge bg-surface px-5 pt-[18px] pb-5">
      <div className="mb-3 flex items-baseline justify-between gap-3">
        <h2 id={labelledBy} className="text-[15px] font-semibold">
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
      className={`inline-flex items-center justify-center rounded-full border font-medium whitespace-nowrap transition-colors focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 disabled:cursor-not-allowed disabled:opacity-40 ${
        small ? "min-h-7 px-3 text-xs" : "min-h-[34px] px-4 text-[13px]"
      } ${
        primary
          ? "border-accent bg-accent font-semibold text-accentink hover:brightness-110"
          : "border-control bg-transparent text-ink hover:border-accent active:translate-y-px"
      }`}
    >
      {children}
    </button>
  );
}
