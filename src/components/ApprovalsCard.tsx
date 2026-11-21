import type { Pending } from "../types";
import { invoke } from "../tauri";
import { Card, ActionButton } from "./ui";
import { Pill } from "./chrome";

function previewArgs(args: Record<string, unknown> | null): string {
  const text = args ? JSON.stringify(args) : "{}";
  return text.length > 160 ? `${text.slice(0, 157)}…` : text;
}

export function ApprovalsCard({
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
    <Card
      labelledBy="approvals-title"
      title="Waiting for you"
      right={<Pill status="pending" label={String(list.length)} />}
    >
      <p className="m-0 text-xs text-dim">Calls held by the Ask policy. They time out in 60s.</p>
      <ul className="mt-1 flex flex-col gap-2.5" aria-live="assertive">
        {list.map((p) => (
          <li
            key={p.id}
            className="flex flex-col gap-2.5 rounded-md border border-[#6b5a2a] bg-raised p-3"
          >
            <span className="font-mono text-[13px] font-semibold text-ink">
              {p.server} / {p.tool}
            </span>
            <code className="block max-h-[4.5em] overflow-auto font-mono text-xs break-words whitespace-pre-wrap text-ink">
              {previewArgs(p.arguments)}
            </code>
            <div className="flex flex-wrap gap-2.5">
              <ActionButton small primary onClick={() => void resolve(p.id, true)}>
                Approve
              </ActionButton>
              <ActionButton small onClick={() => void session(p.id)} title="Approve this tool until the app quits (nothing is saved)">
                Until restart
              </ActionButton>
              <ActionButton small onClick={() => void resolve(p.id, false)}>
                Deny
              </ActionButton>
            </div>
          </li>
        ))}
      </ul>
    </Card>
  );
}
