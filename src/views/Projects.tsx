import type { ServerRow, ToolInfo } from "../types";
import { invoke } from "../tauri";
import { Button, Panel } from "../components/ui";
import { Pill } from "../components/chrome";

function requiredList(tool: ToolInfo): string {
  const schema = tool.input_schema as { required?: unknown } | null;
  const required = Array.isArray(schema?.required) ? (schema.required as unknown[]) : [];
  const names = required.filter((n): n is string => typeof n === "string");
  return names.length > 0 ? `, needs ${names.join(", ")}` : "";
}

function Project({
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

  return (
    <li className="clay-bump px-4 py-3.5">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <span className="font-mono text-[13px] font-[600]">{row.name}</span>
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
      </div>

      {row.error && (
        <p className="mt-2 mb-0 font-mono text-[11.5px] break-words text-bad">{row.error}</p>
      )}

      {row.tools.length > 0 && (
        <details
          className="tools mt-2.5"
          open={row.status === "ready" && row.tools.length <= 3}
        >
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
                <span className="font-mono text-[12px] text-ink">{tool.name}</span>
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

export function Projects({
  rows,
  onServers,
  onAdd,
}: {
  rows: ServerRow[] | null;
  onServers: (rows: ServerRow[]) => void;
  onAdd: () => void;
}) {
  return (
    <div>
      <div className="mb-4 flex items-center justify-between gap-3">
        <h1 className="text-[19px] font-[600] tracking-[-0.018em]">Projects</h1>
        <Button onClick={onAdd}>Add a project</Button>
      </div>
      <Panel labelledBy="projects-title">
      {rows === null ? (
        <p className="m-0 text-[12.5px] text-dim">Loading…</p>
      ) : rows.length === 0 ? (
        <div className="flex flex-col items-start gap-3 py-2">
          <p className="m-0 max-w-[46ch] text-[12.5px] text-dim">
            A project is one MCP server and the tools it gives your chat. Add one from Settings.
          </p>
          <Button onClick={onAdd}>Add a project</Button>
        </div>
      ) : (
        <ul className="m-0 flex list-none flex-col gap-2.5 p-0" aria-live="polite">
          {rows.map((row) => (
            <Project key={row.name} row={row} onServers={onServers} />
          ))}
        </ul>
      )}
      </Panel>
    </div>
  );
}
