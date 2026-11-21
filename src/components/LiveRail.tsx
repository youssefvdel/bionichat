import type { ActivityEntry } from "../types";
import { Pill } from "./chrome";
import type { RunningCall } from "./ToolFlow";

function previewArgs(args: Record<string, unknown> | null): string {
  const text = args ? JSON.stringify(args) : "{}";
  return text.length > 110 ? `${text.slice(0, 107)}…` : text;
}

function Args({ args }: { args: Record<string, unknown> | null }) {
  return (
    <code className="mt-1 block truncate font-mono text-[10.5px] text-dim">
      {previewArgs(args)}
    </code>
  );
}

export function LiveRail({
  running,
  activity,
  bare,
}: {
  running: RunningCall[];
  activity: ActivityEntry[];
  bare?: boolean;
}) {
  return (
    <aside className="flex min-h-0 flex-col gap-3" aria-label="Live calls">
      <div
        className={`flex min-h-0 flex-1 flex-col ${
          bare ? "max-h-[320px] overflow-hidden" : "clay-panel px-4 pt-3.5 pb-4"
        }`}
      >
        <div className="mb-2 flex items-baseline justify-between">
          <h2 className="text-[12.5px] font-[550]">Live calls</h2>
          <Pill
            status={running.length ? "ready" : "idle"}
            label={running.length ? `${running.length} running` : "idle"}
          />
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto pr-1">
          {running.length === 0 && activity.length === 0 && (
            <p className="m-0 text-[11.5px] text-dim">
              Nothing yet. Ask your chat to use a tool and watch it light up here.
            </p>
          )}

          {running.length > 0 && (
            <ul className="m-0 mb-3 flex list-none flex-col gap-2 p-0" aria-live="polite">
              {running.map((call) => (
                <li key={call.key} className="clay-bump px-3 py-2">
                  <div className="flex items-center gap-2">
                    <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-accent" />
                    <span className="truncate font-mono text-[11.5px] font-[550]">
                      {call.server} / {call.tool}
                    </span>
                    <span className="ml-auto shrink-0 text-[10px] text-dim">running</span>
                  </div>
                  <Args args={call.arguments} />
                </li>
              ))}
            </ul>
          )}

          <ul className="m-0 flex list-none flex-col p-0">
            {activity.map((entry, i) => (
              <li
                key={`${entry.time}-${entry.server}-${entry.tool}-${i}`}
                className="border-t border-rule py-2 first:border-t-0"
              >
                <div className="flex items-center gap-2">
                  <span className="truncate font-mono text-[11.5px] font-[550] text-ink">
                    {entry.server} / {entry.tool}
                  </span>
                  <Pill status={entry.status} />
                  <span className="ml-auto shrink-0 text-[10px] text-dim tabular-nums">
                    {entry.time}
                  </span>
                </div>
                <Args args={entry.arguments} />
                {entry.result && (
                  <code className="mt-1 block max-h-[3.2em] overflow-hidden font-mono text-[10.5px] break-words text-sub">
                    {entry.result.length > 160 ? `${entry.result.slice(0, 159)}…` : entry.result}
                  </code>
                )}
                <span className="mt-1 block text-[10px] text-dim tabular-nums">
                  {entry.duration_ms} ms
                </span>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </aside>
  );
}
