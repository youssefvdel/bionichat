import { useEffect, useState } from "react";
import type { ProjectEntry } from "../types";
import { invoke } from "../tauri";
import { Button, Panel } from "../components/ui";

async function copyPath(path: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(path);
    return true;
  } catch {
    return false;
  }
}

export function Projects() {
  const [rows, setRows] = useState<ProjectEntry[] | null>(null);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState("");
  const [reload, setReload] = useState(0);

  useEffect(() => {
    let cancelled = false;
    invoke<ProjectEntry[]>("projects")
      .then((list) => {
        if (!cancelled) {
          setRows(list);
          setError("");
        }
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [reload]);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-3">
        <h1 className="text-[19px] font-[600] tracking-[-0.018em]">Projects</h1>
        <div className="flex items-center gap-2">
          <span className="font-mono text-[11px] text-dim">~/Projects</span>
          <Button small onClick={() => setReload((n) => n + 1)}>
            Refresh
          </Button>
        </div>
      </div>

      {error && (
        <p role="alert" className="m-0 rounded-md bg-[#fdeceb] px-3 py-2 font-mono text-[11.5px] text-bad">
          {error}
        </p>
      )}

      {rows === null ? (
        <Panel>
          <p className="m-0 text-[12.5px] text-dim">Reading your projects folder…</p>
        </Panel>
      ) : rows.length === 0 ? (
        <Panel>
          <p className="m-0 max-w-[46ch] text-[12.5px] text-dim">
            No folders in ~/Projects yet. Create one there and it shows up here.
          </p>
        </Panel>
      ) : (
        <ul className="m-0 grid list-none grid-cols-1 gap-2.5 p-0 md:grid-cols-2">
          {rows.map((row) => (
            <li key={row.path} className="clay-panel flex items-center gap-3 px-4 py-3">
              <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-ground font-mono text-[12px] text-dim">
                {row.name.slice(0, 1).toUpperCase()}
              </span>
              <span className="flex min-w-0 flex-col leading-tight">
                <span className="truncate text-[13px] font-[550] text-ink">{row.name}</span>
                <span className="truncate font-mono text-[10.5px] text-dim">{row.path}</span>
              </span>
              <span className="ml-auto shrink-0 text-[11px] text-dim tabular-nums">
                {row.entries}
              </span>
              <Button
                small
                title="Copy the folder path"
                onClick={() => {
                  void copyPath(row.path).then((ok) => {
                    setCopied(ok ? row.path : "");
                    window.setTimeout(() => setCopied(""), 1400);
                  });
                }}
              >
                {copied === row.path ? "Copied" : "Copy"}
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
