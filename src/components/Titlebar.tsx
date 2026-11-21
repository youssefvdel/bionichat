import { invoke } from "../tauri";
import { Mark } from "./chrome";

function MinIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.2">
      <path d="M1 5h8" />
    </svg>
  );
}

function MaxIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.2">
      <rect x="1" y="1" width="8" height="8" rx="1" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.2">
      <path d="M1 1l8 8M9 1l-8 8" />
    </svg>
  );
}

function WinButton({
  onClick,
  title,
  danger,
  children,
}: {
  onClick: () => void;
  title: string;
  danger?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      onClick={onClick}
      className={`flex h-12 w-12 items-center justify-center text-dim transition-colors hover:bg-raised hover:text-ink ${
        danger ? "hover:bg-bad hover:text-white" : ""
      } focus-visible:outline-2 focus-visible:outline-accent`}
    >
      {children}
    </button>
  );
}

export function Titlebar({ renderer }: { renderer: string }) {
  function run(command: string) {
    void invoke(command).catch(() => {});
  }

  return (
    <header
      data-tauri-drag-region
      className="flex h-12 shrink-0 items-center gap-2.5 pr-0 pl-4 select-none"
    >
      <span className="text-accent">
        <Mark size={18} />
      </span>
      <span className="text-[15px] font-semibold tracking-tight">bionichat</span>
      <span className="mt-0.5 hidden text-xs text-dim sm:inline">Local MCP hub · this PC</span>
      <span className="ml-2 hidden rounded-full bg-raised px-2 py-0.5 font-mono text-[11px] text-dim md:inline">
        {renderer || "—"}
      </span>

      <div className="ml-auto flex items-stretch self-stretch">
        <WinButton title="Minimize" onClick={() => run("window_minimize")}>
          <MinIcon />
        </WinButton>
        <WinButton title="Maximize" onClick={() => run("window_toggle_maximize")}>
          <MaxIcon />
        </WinButton>
        <WinButton title="Close" danger onClick={() => run("window_close")}>
          <CloseIcon />
        </WinButton>
      </div>
    </header>
  );
}
