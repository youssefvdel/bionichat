import type { Pending } from "../types";
import { invoke } from "../tauri";
import { Button, Panel } from "../components/ui";
import { Pill } from "../components/chrome";

function previewArgs(args: Record<string, unknown> | null): string {
  const text = args ? JSON.stringify(args) : "{}";
  return text.length > 160 ? `${text.slice(0, 157)}…` : text;
}

export function Approvals({
  list,
  onChange,
}: {
  list: Pending[];
  onChange: (list: Pending[]) => void;
}) {
  if (list.length === 0) return null;

  async function resolve(id: number, approve: boolean) {
    await invoke<boolean>("resolve_approval", { id, approve });
    onChange(await invoke<Pending[]>("approvals"));
  }

  async function session(id: number) {
    await invoke<boolean>("resolve_session", { id });
    onChange(await invoke<Pending[]>("approvals"));
  }

  return (
    <Panel
      labelledBy="approvals-title"
      title="Waiting for you"
      right={<Pill status="pending" label={String(list.length)} />}
    >
      <p className="m-0 text-[12px] text-dim">
        These calls are held. They time out after 60 seconds.
      </p>
      <ul className="mt-2.5 flex list-none flex-col gap-2.5 p-0" aria-live="assertive">
        {list.map((p) => (
          <li key={p.id} className="clay-bump flex flex-col gap-2.5 p-3.5">
            <span className="font-mono text-[12.5px] font-[550] text-ink">
              {p.server} / {p.tool}
            </span>
            <code className="block max-h-[4.5em] overflow-auto font-mono text-[11.5px] break-words whitespace-pre-wrap text-sub">
              {previewArgs(p.arguments)}
            </code>
            <div className="flex flex-wrap gap-2">
              <Button small primary onClick={() => void resolve(p.id, true)}>
                Approve
              </Button>
              <Button
                small
                onClick={() => void session(p.id)}
                title="Approve this tool until the app quits (nothing is saved)"
              >
                Until restart
              </Button>
              <Button small onClick={() => void resolve(p.id, false)}>
                Deny
              </Button>
            </div>
          </li>
        ))}
      </ul>
    </Panel>
  );
}
