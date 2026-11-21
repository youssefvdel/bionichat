import { useRef, useState } from "react";
import type { ServerRow } from "../types";
import { invoke } from "../tauri";
import { Card, ActionButton } from "./ui";

export function AddCard({ onServers }: { onServers: (rows: ServerRow[]) => void }) {
  const [json, setJson] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  async function submit() {
    const text = json.trim();
    if (!text) {
      setError("paste some JSON first");
      return;
    }
    setError("");
    setBusy(true);
    try {
      onServers(await invoke<ServerRow[]>("add_servers", { json: text }));
      setJson("");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  function importFile(file: File | undefined) {
    if (!file) return;
    void file
      .text()
      .then((text) => {
        setJson(text);
        setError("");
      })
      .catch((e: unknown) => setError(`cannot read file: ${String(e)}`));
  }

  return (
    <Card
      labelledBy="add-title"
      title="Add MCP servers"
      right={<span className="text-xs text-dim">Anthropic JSON</span>}
    >
      <p className="m-0 text-xs text-dim">
        Paste a <code className="font-mono text-xs text-dim">mcpServers</code> object or import a{" "}
        <code className="font-mono text-xs text-dim">claude_desktop_config.json</code>. Names already
        registered are replaced.
      </p>
      <textarea
        value={json}
        onChange={(e) => setJson(e.target.value)}
        spellCheck={false}
        rows={7}
        placeholder={`{
  "mcpServers": {
    "everything": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-everything"] }
  }
}`}
        className="mt-3 w-full resize-y rounded-md border border-control bg-raised p-2.5 font-mono text-xs leading-relaxed text-ink placeholder:text-dim focus-visible:outline-2 focus-visible:outline-accent"
      />
      <div className="mt-3 flex flex-wrap items-center gap-2.5">
        <ActionButton primary disabled={busy} onClick={() => void submit()}>
          {busy ? "Spawning…" : "Add and spawn"}
        </ActionButton>
        <label className="inline-flex min-h-[34px] cursor-pointer items-center rounded-full border border-control bg-transparent px-4 text-[13px] font-medium text-ink transition-colors hover:border-accent focus-within:outline-2 focus-within:outline-accent">
          Import file
          <input
            ref={fileRef}
            type="file"
            accept="application/json,.json"
            hidden
            onChange={(e) => {
              importFile(e.target.files?.[0]);
              e.target.value = "";
            }}
          />
        </label>
        {error && (
          <span role="alert" className="font-mono text-xs break-words text-bad">
            {error}
          </span>
        )}
      </div>
    </Card>
  );
}
