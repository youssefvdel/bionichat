import { useState } from "react";
import { Pill } from "./chrome";

/** Strip the time stamp the hub appends, so a preview shows only the payload. */
function stripStamp(text: string): string {
  const at = text.indexOf("\n\n[server time ");
  return at === -1 ? text : text.slice(0, at);
}

function str(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function bash(report: string): { stdout: string; stderr: string; code: number | null } {
  try {
    const parsed = JSON.parse(stripStamp(report)) as {
      exit_code?: number | null;
      stdout?: string;
      stderr?: string;
    };
    return {
      stdout: parsed.stdout ?? "",
      stderr: parsed.stderr ?? "",
      code: typeof parsed.exit_code === "number" ? parsed.exit_code : null,
    };
  } catch {
    return { stdout: stripStamp(report), stderr: "", code: null };
  }
}

/** What to show above the output: the command, the path, the skill name. */
function headline(tool: string, args: Record<string, unknown> | null): string {
  if (!args) return "";
  switch (tool) {
    case "bash":
      return str(args.command);
    case "read":
    case "write":
    case "edit":
      return str(args.path);
    case "skill":
      return str(args.name) || "list";
    default:
      return JSON.stringify(args);
  }
}

function kindOf(server: string, tool: string): string {
  if (tool === "bash") return "Shell";
  if (tool === "read") return "Read";
  if (tool === "write") return "Write";
  if (tool === "edit") return "Edit";
  if (server === "skill") return "Skill";
  return server;
}

/**
 * A tool call as a person reads it: what ran, then what came back.
 * Raw argument JSON is never shown unless the tool is unknown.
 */
export function CallPreview({
  server,
  tool,
  args,
  result,
  status,
  running,
  duration,
  time,
}: {
  server: string;
  tool: string;
  args: Record<string, unknown> | null;
  result: string | null;
  status?: string;
  running?: boolean;
  duration?: number;
  time?: string;
}) {
  const [open, setOpen] = useState(false);
  const line = headline(tool, args);

  const report = result && tool === "bash" ? bash(result) : null;
  const output = report
    ? [report.stdout.trimEnd(), report.stderr.trim() ? `stderr:\n${report.stderr.trimEnd()}` : ""]
        .filter(Boolean)
        .join("\n")
    : result
      ? stripStamp(result).trim()
      : "";

  // `read` repeats the path as its first line; the headline already shows it.
  const body =
    tool === "read" && output.split("\n")[0]?.includes(" (lines ")
      ? output.split("\n").slice(1).join("\n")
      : output;

  const long = body.length > 320 || body.split("\n").length > 7;
  const shown = long && !open ? body.split("\n").slice(0, 7).join("\n") : body;

  return (
    <div className="clay-bump overflow-hidden">
      <div className="flex items-center gap-2 px-3 py-2">
        <span className="text-[11px] font-[600] tracking-[0.02em] text-sub">
          {kindOf(server, tool)}
        </span>
        <span className="min-w-0 truncate font-mono text-[11px] text-dim">
          {server !== kindOf(server, tool) && server !== tool ? server : ""}
        </span>
        <span className="ml-auto flex shrink-0 items-center gap-1.5">
          {running ? (
            <span className="text-[10.5px] text-dim">running</span>
          ) : status ? (
            <Pill status={status} />
          ) : null}
          {typeof duration === "number" && (
            <span className="text-[10px] text-dim tabular-nums">{duration} ms</span>
          )}
          {time && <span className="text-[10px] text-dim tabular-nums">{time}</span>}
        </span>
      </div>

      {line && (
        <div className="flex gap-1.5 border-t border-rule bg-surface px-3 py-1.5">
          <span className="shrink-0 font-mono text-[11px] text-dim">
            {tool === "bash" ? "$" : ""}
          </span>
          <code className="min-w-0 overflow-x-auto font-mono text-[11px] whitespace-pre text-ink">
            {line}
          </code>
        </div>
      )}

      {shown && (
        <div className="border-t border-rule bg-surface/60">
          <pre className="m-0 max-h-[132px] overflow-auto px-3 py-2 font-mono text-[10.5px] leading-[1.5] whitespace-pre-wrap text-sub">
            {shown}
          </pre>
          {long && (
            <button
              type="button"
              onClick={() => setOpen((was) => !was)}
              className="w-full border-t border-rule px-3 py-1 text-left text-[10.5px] text-dim hover:text-ink"
            >
              {open ? "Show less" : `Show all (${body.split("\n").length} lines)`}
            </button>
          )}
        </div>
      )}

      {report?.code != null && report.code !== 0 && (
        <div className="border-t border-rule px-3 py-1 font-mono text-[10.5px] text-bad">
          exit {report.code}
        </div>
      )}
    </div>
  );
}
