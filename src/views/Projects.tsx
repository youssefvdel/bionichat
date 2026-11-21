import { useEffect, useState } from "react";
import type { BrowseView, ProjectEntry } from "../types";
import { invoke } from "../tauri";
import { Button, Panel } from "../components/ui";

function FolderPicker({
  onPick,
  onClose,
}: {
  onPick: (path: string) => void;
  onClose: () => void;
}) {
  const [view, setView] = useState<BrowseView | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    let cancelled = false;
    invoke<BrowseView>("browse", { path: null })
      .then((next) => {
        if (!cancelled) setView(next);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  function go(path: string | null) {
    setError("");
    invoke<BrowseView>("browse", { path })
      .then(setView)
      .catch((e: unknown) => setError(String(e)));
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-ink/25 p-6"
      role="dialog"
      aria-modal="true"
      aria-label="Choose a folder"
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
    >
      <div className="clay-panel flex max-h-[520px] w-full max-w-[560px] flex-col overflow-hidden">
        <div className="flex items-baseline justify-between gap-3 px-5 pt-4">
          <h2 className="text-[14px] font-[600]">Choose a folder</h2>
          <button
            type="button"
            onClick={onClose}
            className="text-[11.5px] text-dim hover:text-ink"
          >
            Close
          </button>
        </div>

        <div className="flex items-center gap-2 px-5 pt-2 pb-3">
          <Button small disabled={!view?.parent} onClick={() => go(view?.parent ?? null)}>
            Up
          </Button>
          <code className="min-w-0 flex-1 truncate font-mono text-[11px] text-sub">
            {view?.path ?? "…"}
          </code>
        </div>

        <ul className="m-0 min-h-0 flex-1 list-none overflow-y-auto border-t border-rule p-2">
          {view?.dirs.length === 0 && (
            <li className="px-3 py-2 text-[12px] text-dim">No sub-folders here.</li>
          )}
          {view?.dirs.map((dir) => (
            <li key={dir.path}>
              <button
                type="button"
                onDoubleClick={() => go(dir.path)}
                onClick={() => go(dir.path)}
                className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-[12.5px] hover:bg-ground"
                title="Open this folder"
              >
                <span className="text-dim">▸</span>
                <span className="truncate font-mono text-[12px]">{dir.name}</span>
              </button>
            </li>
          ))}
        </ul>

        {error && (
          <p className="m-0 border-t border-rule px-5 py-2 font-mono text-[11px] text-bad">
            {error}
          </p>
        )}

        <div className="flex items-center justify-between gap-3 border-t border-rule px-5 py-3">
          <span className="text-[11px] text-dim">
            Open a folder to look inside, or use the one you are in.
          </span>
          <Button primary disabled={!view} onClick={() => view && onPick(view.path)}>
            Use this folder
          </Button>
        </div>
      </div>
    </div>
  );
}

export function Projects() {
  const [rows, setRows] = useState<ProjectEntry[] | null>(null);
  const [error, setError] = useState("");
  const [picking, setPicking] = useState(false);

  useEffect(() => {
    invoke<ProjectEntry[]>("projects")
      .then(setRows)
      .catch((e: unknown) => setError(String(e)));
  }, []);

  async function add(path: string) {
    try {
      setRows(await invoke<ProjectEntry[]>("add_project", { path }));
      setError("");
      setPicking(false);
    } catch (e) {
      setError(String(e));
    }
  }

  async function remove(path: string) {
    setRows(await invoke<ProjectEntry[]>("remove_project", { path }));
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-3">
        <h1 className="text-[19px] font-[600] tracking-[-0.018em]">Projects</h1>
        <Button primary onClick={() => setPicking(true)}>
          Choose a folder
        </Button>
      </div>

      {error && (
        <p
          role="alert"
          className="m-0 rounded-md bg-[#fdeceb] px-3 py-2 font-mono text-[11.5px] text-bad"
        >
          {error}
        </p>
      )}

      {rows === null ? (
        <Panel>
          <p className="m-0 text-[12.5px] text-dim">Reading your projects…</p>
        </Panel>
      ) : rows.length === 0 ? (
        <Panel>
          <div className="flex flex-col items-start gap-3 py-2">
            <p className="m-0 max-w-[48ch] text-[12.5px] text-dim">
              A project is a folder you work in. Pick one anywhere on this machine and a chat will
              run against it.
            </p>
            <Button onClick={() => setPicking(true)}>Choose a folder</Button>
          </div>
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
              <Button small onClick={() => void remove(row.path)} title={`Remove ${row.name}`}>
                Remove
              </Button>
            </li>
          ))}
        </ul>
      )}

      {picking && <FolderPicker onPick={(path) => void add(path)} onClose={() => setPicking(false)} />}
    </div>
  );
}
