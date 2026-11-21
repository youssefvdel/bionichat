import { invoke } from "../tauri";
import { Mark } from "./chrome";

function MinIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round">
      <path d="M1.5 5h7" />
    </svg>
  );
}

function MaxIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.4">
      <rect x="1.2" y="1.2" width="7.6" height="7.6" rx="1.6" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round">
      <path d="M1.6 1.6l6.8 6.8M8.4 1.6l-6.8 6.8" />
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
      className={`clay-button clay-button-sm !min-h-8 !w-8 !px-0 text-dim ${
        danger ? "hover:!bg-[#fdecea] hover:!text-bad" : "hover:!text-ink"
      }`}
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
      className="flex h-[54px] shrink-0 items-center gap-2.5 pr-3 pl-5 select-none"
    >
      <span className="text-ink">
        <Mark size={17} />
      </span>
      <span className="text-[14px] font-[600] tracking-[-0.01em]">bionichat</span>
      <span className="mt-px hidden text-[12px] text-dim sm:inline">
        your PC, reachable from the chat
      </span>
      <span className="ml-1 hidden rounded-full bg-raised px-2 py-1 font-mono text-[10.5px] text-dim md:inline">
        {renderer || "—"}
      </span>

      <div className="ml-auto flex items-center gap-1.5">
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
