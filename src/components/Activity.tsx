import type { ActivityEntry } from "../types";
import { Panel } from "../components/ui";
import { Pill } from "../components/chrome";

export function Activity({ feed }: { feed: ActivityEntry[] }) {
  return (
    <Panel
      labelledBy="activity-title"
      title="Activity"
      right={<Pill status="idle" label={feed.length ? String(feed.length) : "idle"} />}
    >
      {feed.length === 0 ? (
        <p className="m-0 text-[12.5px] text-dim">
          Nothing yet. The first tool call from your chat shows up here.
        </p>
      ) : (
        <ul className="mt-1 flex list-none flex-col p-0" aria-live="polite">
          {feed.map((entry, i) => (
            <li
              key={`${entry.time}-${entry.server}-${entry.tool}-${i}`}
              className="border-t border-rule py-2 first:border-t-0"
            >
              <div className="animate-row-in flex min-h-6 items-center gap-2.5">
                <span className="font-mono text-[12px] font-[550] whitespace-nowrap text-ink">
                  {entry.server} / {entry.tool}
                </span>
                <Pill status={entry.status} />
                <span className="ml-auto text-[11.5px] text-dim tabular-nums">
                  {entry.time} · {entry.duration_ms}ms
                </span>
              </div>
              {entry.result && (
                <code className="mt-0.5 block font-mono text-[11.5px] break-words whitespace-pre-wrap text-dim">
                  {entry.result.length > 220 ? `${entry.result.slice(0, 219)}…` : entry.result}
                </code>
              )}
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}
