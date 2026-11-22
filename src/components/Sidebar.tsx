import { Mark, OverviewIcon, ProjectsIcon, SettingsIcon } from "./chrome";

export type View = "overview" | "projects" | "settings";

const NAV: Array<{
  id: View;
  label: string;
  Icon: () => React.JSX.Element;
}> = [
  { id: "overview", label: "Overview", Icon: OverviewIcon },
  { id: "projects", label: "Projects", Icon: ProjectsIcon },
];

export function Sidebar({
  view,
  onView,
  waiting,
  version,
}: {
  view: View;
  onView: (view: View) => void;
  waiting: number;
  version: string;
}) {
  return (
    <nav
      aria-label="Main"
      className="flex w-[188px] shrink-0 flex-col gap-0.5 bg-[#e9e9ee] px-2.5 pt-2 pb-3"
    >
      {NAV.map(({ id, label, Icon }) => (
        <button
          key={id}
          type="button"
          className="clay-nav-item"
          data-active={view === id}
          aria-current={view === id ? "page" : undefined}
          onClick={() => onView(id)}
        >
          <Icon />
          <span>{label}</span>
          {id === "overview" && waiting > 0 && (
            <span className="ml-auto rounded-full bg-[#fdf4e6] px-1.5 py-0.5 font-mono text-[10px] text-warn tabular-nums">
              {waiting}
            </span>
          )}
        </button>
      ))}

      <div className="mt-auto flex flex-col gap-0.5">
        <button
          type="button"
          className="clay-nav-item"
          data-active={view === "settings"}
          aria-current={view === "settings" ? "page" : undefined}
          onClick={() => onView("settings")}
        >
          <SettingsIcon />
          <span>Settings</span>
        </button>

        <button
          type="button"
          className="mt-1 flex items-center gap-2 rounded-xl px-2 py-2 text-left transition-colors hover:bg-white/60"
          title={`bionichat ${version}`}
          onClick={() => onView("overview")}
        >
          <span className="text-ink/70">
            <Mark size={15} />
          </span>
          <span className="flex flex-col leading-tight">
            <span className="text-[11.5px] font-[550] text-sub">bionichat</span>
            <span className="font-mono text-[10px] text-dim">{version}</span>
          </span>
        </button>
      </div>
    </nav>
  );
}
