import { OverviewIcon, Pill, ProjectsIcon, SettingsIcon } from "./chrome";

export type View = "overview" | "projects" | "settings";

const NAV: Array<{
  id: View;
  label: string;
  Icon: () => React.JSX.Element;
}> = [
  { id: "overview", label: "Overview", Icon: OverviewIcon },
  { id: "projects", label: "Projects", Icon: ProjectsIcon },
  { id: "settings", label: "Settings", Icon: SettingsIcon },
];

export function Sidebar({
  view,
  onView,
  waiting,
}: {
  view: View;
  onView: (view: View) => void;
  waiting: number;
}) {
  return (
    <nav
      aria-label="Main"
      className="flex w-[196px] shrink-0 flex-col gap-1 px-3 pt-2 pb-4"
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
        </button>
      ))}

      <div className="mt-auto flex flex-col gap-2 px-3 pt-6 text-[11.5px] text-dim">
        {waiting > 0 && <Pill status="pending" label={`${waiting} to approve`} />}
      </div>
    </nav>
  );
}
