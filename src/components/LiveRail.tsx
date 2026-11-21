import type { ActivityEntry } from "../types";
import { Pill } from "./chrome";
import { CallPreview } from "./CallPreview";
import type { RunningCall } from "./ToolFlow";

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
          bare ? "max-h-[440px] overflow-hidden" : "clay-panel px-4 pt-3.5 pb-4"
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
            <ul className="m-0 mb-2 flex list-none flex-col gap-2 p-0" aria-live="polite">
              {running.map((call) => (
                <li key={call.key}>
                  <CallPreview
                    server={call.server}
                    tool={call.tool}
                    args={call.arguments}
                    result={null}
                    running
                  />
                </li>
              ))}
            </ul>
          )}

          <ul className="m-0 flex list-none flex-col gap-2 p-0">
            {activity.map((entry, i) => (
              <li key={`${entry.time}-${entry.server}-${entry.tool}-${i}`}>
                <CallPreview
                  server={entry.server}
                  tool={entry.tool}
                  args={entry.arguments}
                  result={entry.result}
                  status={entry.status}
                  duration={entry.duration_ms}
                  time={entry.time}
                />
              </li>
            ))}
          </ul>
        </div>
      </div>
    </aside>
  );
}
