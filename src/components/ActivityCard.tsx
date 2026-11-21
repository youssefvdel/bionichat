import type { ActivityEntry } from "../types";
import { Card } from "./ui";
import { Pill } from "./chrome";

export function ActivityCard({ feed }: { feed: ActivityEntry[] }) {
  if (feed.length === 0) return null;
  return (
    <Card
      labelledBy="activity-title"
      title="Live activity"
      right={<Pill status="idle" label={String(feed.length)} />}
    >
      <p className="m-0 text-xs text-dim">Every tool call as it happens, newest first.</p>
      <ul className="mt-1 flex flex-col" aria-live="polite">
        {feed.map((entry, i) => (
          <li
            key={`${entry.time}-${entry.server}-${entry.tool}-${i}`}
            className="border-t border-rule py-1.5 first:border-t-0"
          >
            <div className="flex min-h-7 items-center gap-2.5">
              <span className="font-mono text-xs font-semibold whitespace-nowrap text-ink">
                {entry.server} / {entry.tool}
              </span>
              <Pill status={entry.status} />
              <span className="text-xs text-dim tabular-nums">
                {entry.time} · {entry.duration_ms}ms
              </span>
            </div>
            {entry.result && (
              <code className="mt-0.5 block font-mono text-xs break-words whitespace-pre-wrap text-dim">
                {entry.result.length > 220 ? `${entry.result.slice(0, 219)}…` : entry.result}
              </code>
            )}
          </li>
        ))}
      </ul>
    </Card>
  );
}
