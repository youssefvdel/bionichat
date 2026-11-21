import type { ServerRow, ToolInfo } from "../types";
import { invoke } from "../tauri";
import { Card, ActionButton } from "./ui";
import { Pill } from "./chrome";

const POLICY_LABELS: Array<[string, string]> = [
  ["allow", "Allow"],
  ["ask", "Ask"],
  ["deny", "Deny"],
];

function requiredList(tool: ToolInfo): string {
  const schema = tool.input_schema as { required?: unknown } | null;
  const required = Array.isArray(schema?.required) ? (schema.required as unknown[]) : [];
  const names = required.filter((n): n is string => typeof n === "string");
  return names.length > 0 ? `, needs ${names.join(", ")}` : "";
}

function ServerNode({
  row,
  onServers,
}: {
  row: ServerRow;
  onServers: (rows: ServerRow[]) => void;
}) {
  async function restart() {
    onServers(await invoke<ServerRow[]>("restart_server", { server: row.name }));
  }

  async function policy(server: string, tool: string, decision: string) {
    onServers(await invoke<ServerRow[]>("set_policy", { server, tool, decision }));
  }

  async function autorestart(enabled: boolean) {
    onServers(await invoke<ServerRow[]>("set_autorestart", { server: row.name, enabled }));
  }

  return (
    <li className="overflow-hidden rounded-md border border-edge bg-raised">
      <div className="flex min-h-[42px] flex-wrap items-center gap-x-2.5 gap-y-1.5 px-3 py-2">
        <span className="font-mono text-[13px] font-semibold whitespace-nowrap">{row.name}</span>
        <span className="min-w-0 flex-1 overflow-hidden font-mono text-xs text-ellipsis whitespace-nowrap text-dim">
          {row.transport}
        </span>
        {row.secret_keys.length > 0 && (
          <span className="flex gap-1">
            {row.secret_keys.map((key) => (
              <span
                key={key}
                className="rounded-full border border-edge px-2 py-0.5 font-mono text-[11px] text-dim tabular-nums"
              >
                {key} ••••
              </span>
            ))}
          </span>
        )}
        <Pill status={row.status} />
        <ActionButton small onClick={() => void restart()}>
          Restart
        </ActionButton>
        <label className="inline-flex cursor-pointer items-center gap-1.5 text-xs whitespace-nowrap text-dim">
          <input
            type="checkbox"
            className="accent-accent"
            checked={row.autorestart}
            onChange={(e) => void autorestart(e.target.checked)}
          />
          auto-restart
        </label>
      </div>
      {row.error && <p className="m-0 px-3 pb-2.5 font-mono text-xs break-words text-bad">{row.error}</p>}
      {row.tools.length > 0 && (
        <details className="tools border-t border-rule" open={row.status === "ready" && row.tools.length <= 3}>
          <summary className="flex min-h-9 cursor-pointer items-center px-3 text-xs text-dim">
            {row.tools.length} tool{row.tools.length === 1 ? "" : "s"}
          </summary>
          <ul className="m-0 flex list-none flex-col gap-1.5 px-3 pt-0 pb-3">
            {row.tools.map((tool) => (
              <li
                key={tool.name}
                title={`${tool.name}${requiredList(tool)}`}
                className="flex min-h-7 items-baseline gap-2 text-xs"
              >
                <span className="font-mono text-xs text-ink">{tool.name}</span>
                {tool.read_only && (
                  <span className="rounded-full border border-edge px-2 py-0.5 font-mono text-[11px] text-dim">
                    read-only
                  </span>
                )}
                {tool.description && <span className="text-xs text-dim">{tool.description}</span>}
                <select
                  className="ml-auto h-7 cursor-pointer rounded-full border border-control bg-surface px-2 font-[inherit] text-xs text-ink"
                  title={`Policy for ${row.name} / ${tool.name}`}
                  value={tool.policy}
                  onChange={(e) => void policy(row.name, tool.name, e.target.value)}
                >
                  {POLICY_LABELS.map(([value, label]) => (
                    <option key={value} value={value}>
                      {label}
                    </option>
                  ))}
                </select>
              </li>
            ))}
          </ul>
        </details>
      )}
    </li>
  );
}

export function ServersCard({
  rows,
  approvalsDefault,
  onServers,
}: {
  rows: ServerRow[] | null;
  approvalsDefault: string;
  onServers: (rows: ServerRow[]) => void;
}) {
  async function refreshServers() {
    onServers(await invoke<ServerRow[]>("refresh"));
  }
  return (
    <Card
      labelledBy="servers-title"
      title={
        <>
          Servers{" "}
          <span className="inline-block min-w-5 rounded-full bg-raised px-1.5 text-center align-[1px] font-mono text-[11px] text-dim">
            {rows?.length ?? 0}
          </span>
        </>
      }
      right={
        <span className="flex items-center gap-2.5 text-xs text-dim">
          approvals default: <strong className="text-ink">{approvalsDefault || "ask"}</strong>
          <ActionButton small onClick={() => void refreshServers()}>Refresh</ActionButton>
        </span>
      }
    >
      {rows === null ? (
        <p className="m-0 text-dim">Loading registry…</p>
      ) : rows.length === 0 ? (
        <p className="m-0 text-dim">No servers registered yet. Paste JSON below.</p>
      ) : (
        <ul className="m-0 flex list-none flex-col gap-3 p-0" aria-live="polite">
          {rows.map((row) => (
            <ServerNode key={row.name} row={row} onServers={onServers} />
          ))}
        </ul>
      )}
    </Card>
  );
}
