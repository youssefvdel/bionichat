import { useState } from "react";
import type { ServerRow, ToolInfo } from "../types";
import { invoke } from "../tauri";
import { Button } from "./ui";
import { Pill } from "./chrome";

function requiredList(tool: ToolInfo): string {
  const schema = tool.input_schema as { required?: unknown } | null;
  const required = Array.isArray(schema?.required) ? (schema.required as unknown[]) : [];
  const names = required.filter((n): n is string => typeof n === "string");
  return names.length > 0 ? `, needs ${names.join(", ")}` : "";
}

function Server({
  row,
  onServers,
}: {
  row: ServerRow;
  onServers: (rows: ServerRow[]) => void;
}) {
  async function restart() {
    onServers(await invoke<ServerRow[]>("restart_server", { server: row.name }));
  }

  async function policy(tool: string, decision: string) {
    onServers(await invoke<ServerRow[]>("set_policy", { server: row.name, tool, decision }));
  }

  async function autorestart(enabled: boolean) {
    onServers(await invoke<ServerRow[]>("set_autorestart", { server: row.name, enabled }));
  }

  async function remove() {
    onServers(await invoke<ServerRow[]>("remove_server", { name: row.name }));
  }

  return (
    <li className="clay-bump px-4 py-3.5">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <span className="font-mono text-[13px] font-[600]">{row.name}</span>
        {row.namespace !== row.name && (
          <span className="rounded-full bg-ground px-2 py-0.5 font-mono text-[10px] text-dim">
            {row.namespace}_
          </span>
        )}
        <span className="min-w-0 flex-1 truncate font-mono text-[11.5px] text-dim">
          {row.transport}
        </span>
        <Pill status={row.status} />
        <Button small onClick={() => void restart()}>
          Restart
        </Button>
        <label className="inline-flex cursor-pointer items-center gap-1.5 text-[11.5px] whitespace-nowrap text-dim">
          <input
            type="checkbox"
            className="accent-accent"
            checked={row.autorestart}
            onChange={(e) => void autorestart(e.target.checked)}
          />
          auto-restart
        </label>
        {!row.builtin && <RemoveButton name={row.name} onConfirm={() => void remove()} />}
      </div>

      {row.error && (
        <p className="mt-2 mb-0 font-mono text-[11.5px] break-words text-bad">{row.error}</p>
      )}

      {row.tools.length > 0 && (
        <details className="tools mt-2.5" open={row.status === "ready" && row.tools.length <= 3}>
          <summary className="flex min-h-7 cursor-pointer items-center text-[11.5px] text-dim">
            {row.tools.length} tool{row.tools.length === 1 ? "" : "s"}
          </summary>
          <ul className="m-0 flex list-none flex-col gap-1.5 p-0 pt-1">
            {row.tools.map((tool) => (
              <li
                key={tool.name}
                title={`${tool.name}${requiredList(tool)}`}
                className="flex min-h-7 items-baseline gap-2 text-[12px]"
              >
                <span className="font-mono text-[12px] text-ink">
                  {row.namespace}_{tool.name}
                </span>
                {tool.read_only && (
                  <span className="rounded-full bg-ground px-2 py-0.5 font-mono text-[10.5px] text-dim">
                    read-only
                  </span>
                )}
                {tool.description && (
                  <span className="min-w-0 flex-1 truncate text-[11.5px] text-dim">
                    {tool.description}
                  </span>
                )}
                <select
                  className="clay-select ml-auto"
                  title={`Policy for ${row.name} / ${tool.name}`}
                  value={tool.policy}
                  onChange={(e) => void policy(tool.name, e.target.value)}
                >
                  <option value="allow">Allow</option>
                  <option value="ask">Ask</option>
                  <option value="deny">Deny</option>
                </select>
              </li>
            ))}
          </ul>
        </details>
      )}
    </li>
  );
}

function RemoveButton({ name, onConfirm }: { name: string; onConfirm: () => void }) {
  const [armed, setArmed] = useState(false);
  return (
    <button
      type="button"
      title={armed ? "Click again to remove this server and its tools" : `Remove ${name}`}
      onClick={() => {
        if (!armed) {
          setArmed(true);
          window.setTimeout(() => setArmed(false), 5000);
          return;
        }
        setArmed(false);
        onConfirm();
      }}
      className={`clay-button clay-button-sm shrink-0 whitespace-nowrap !px-2.5 ${
        armed ? "!text-bad" : "text-dim"
      }`}
    >
      {armed ? "Sure?" : "Remove"}
    </button>
  );
}

export function Servers({
  rows,
  onServers,
}: {
  rows: ServerRow[] | null;
  onServers: (rows: ServerRow[]) => void;
}) {
  if (rows === null) {
    return <p className="m-0 text-[12.5px] text-dim">Loading…</p>;
  }
  if (rows.length === 0) {
    return (
      <p className="m-0 max-w-[46ch] text-[12.5px] text-dim">
        No servers yet. Add one below with Anthropic JSON.
      </p>
    );
  }
  return (
    <ul className="m-0 flex list-none flex-col gap-2.5 p-0" aria-live="polite">
      {rows.map((row) => (
        <Server key={row.name} row={row} onServers={onServers} />
      ))}
    </ul>
  );
}
