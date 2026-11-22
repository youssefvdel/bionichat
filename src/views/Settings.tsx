import { useEffect, useState } from "react";
import type { ActivityEntry, ModelView, ServerRow, TunnelInfo } from "../types";
import { invoke } from "../tauri";
import { Button, Panel } from "../components/ui";
import { Pill } from "../components/chrome";
import { LiveRail } from "../components/LiveRail";
import { ToolFlow, type FlashState, type RunningCall } from "../components/ToolFlow";
import { Servers } from "../components/Servers";

function Tunnel({
  info,
  onChange,
  primary,
}: {
  info: TunnelInfo;
  onChange: (info: TunnelInfo) => void;
  primary: boolean;
}) {
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState("");
  const [armed, setArmed] = useState(false);
  const [copied, setCopied] = useState(false);

  const busy = info.status === "starting" || starting;
  const live = info.status === "ready";

  async function run(work: Promise<TunnelInfo>) {
    setError("");
    try {
      onChange(await work);
    } catch (e) {
      setError(String(e));
    }
  }

  async function start() {
    setStarting(true);
    await run(invoke<TunnelInfo>("tunnel_start"));
    setStarting(false);
  }

  function revoke() {
    if (!armed) {
      setArmed(true);
      window.setTimeout(() => setArmed(false), 6000);
      return;
    }
    setArmed(false);
    void run(invoke<TunnelInfo>("tunnel_revoke"));
  }

  return (
    <Panel
      labelledBy="tunnel-title"
      title="Tunnel"
      right={<Pill status={busy ? "starting" : info.status} />}
    >
      <p className="m-0 text-[12.5px] text-dim">
        {live
          ? "This machine is reachable from the public internet. Anyone holding the link can call your tools, so gate the sensitive ones below."
          : "No public link. Your chat cannot reach this machine until the tunnel starts."}
      </p>

      <code className="mt-2.5 block font-mono text-[15px] break-all text-ink">
        {info.public_url ?? "no public link yet"}
      </code>

      <div className="mt-3 flex flex-wrap items-center gap-2">
        {live ? (
          <>
            <Button
              primary={primary}
              onClick={() => {
                void navigator.clipboard
                  .writeText(info.public_url ?? "")
                  .then(() => {
                    setCopied(true);
                    window.setTimeout(() => setCopied(false), 1400);
                  })
                  .catch(() => {});
              }}
            >
              {copied ? "Copied" : "Copy link"}
            </Button>
            <Button onClick={() => void run(invoke<TunnelInfo>("tunnel_stop"))}>Stop</Button>
            <Button onClick={revoke} title="Burn this link for good; next start mints a new one">
              {armed ? "Sure? Click again" : "Revoke link"}
            </Button>
          </>
        ) : (
          <Button primary={primary} disabled={busy} onClick={() => void start()}>
            {busy ? "Starting…" : "Start tunnel"}
          </Button>
        )}
      </div>

      {(error || info.error) && (
        <p role="alert" className="mt-2 font-mono text-[11.5px] break-words text-bad">
          {error || info.error}
        </p>
      )}
    </Panel>
  );
}

function ModelPanel() {
  const [view, setView] = useState<ModelView | null>(null);
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [key, setKey] = useState("");
  const [note, setNote] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    invoke<ModelView>("model_config")
      .then((next) => {
        setView(next);
        setBaseUrl(next.base_url);
        setModel(next.model);
      })
      .catch((e: unknown) => setError(String(e)));
  }, []);

  async function save() {
    setError("");
    setNote("");
    try {
      const next = await invoke<ModelView>("save_model", {
        baseUrl,
        model,
        apiKey: key,
      });
      setView(next);
      setKey("");
      setNote(next.configured ? "saved" : "saved, but no key yet");
      window.setTimeout(() => setNote(""), 1800);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <Panel
      labelledBy="model-title"
      title="Model"
      right={
        <span className="text-[11.5px] text-dim">
          {view?.configured ? `ready · ${view.key_hint}` : "not configured"}
        </span>
      }
    >
      <p className="m-0 text-[12.5px] text-dim">
        The chat runs on this machine and calls any OpenAI-compatible API. The key stays in
        <code className="mx-1 font-mono text-[11.5px]">model.json</code>
        with owner-only permissions and never reaches the page.
      </p>

      <div className="mt-3 grid grid-cols-1 gap-2 md:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)]">
        <label className="flex flex-col gap-1 text-[11px] text-dim">
          Base URL
          <input
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            spellCheck={false}
            placeholder="https://api.openai.com/v1"
            className="clay-field font-mono !text-[11.5px]"
          />
        </label>
        <label className="flex flex-col gap-1 text-[11px] text-dim">
          Model
          <input
            value={model}
            onChange={(e) => setModel(e.target.value)}
            spellCheck={false}
            placeholder="gpt-4o-mini"
            className="clay-field font-mono !text-[11.5px]"
          />
        </label>
      </div>

      <label className="mt-2 flex flex-col gap-1 text-[11px] text-dim">
        API key
        <input
          value={key}
          onChange={(e) => setKey(e.target.value)}
          type="password"
          spellCheck={false}
          placeholder={view?.key_hint ? `stored ${view.key_hint}, type to replace` : "sk-…"}
          className="clay-field font-mono !text-[11.5px]"
        />
      </label>

      <div className="mt-3 flex items-center gap-2">
        <Button onClick={() => void save()}>Save model</Button>
        {note && <span className="text-[11.5px] text-good">{note}</span>}
        {error && (
          <span role="alert" className="font-mono text-[11.5px] break-words text-bad">
            {error}
          </span>
        )}
      </div>
    </Panel>
  );
}

function AddServer({
  onServers,
  primary,
}: {
  onServers: (rows: ServerRow[]) => void;
  primary: boolean;
}) {
  const [json, setJson] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

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

  return (
    <Panel
      labelledBy="add-title"
      title="Add a server"
      right={<span className="text-[11.5px] text-dim">Anthropic JSON</span>}
    >
      <p className="m-0 text-[12.5px] text-dim">
        Paste an <code className="font-mono text-[11.5px]">mcpServers</code> object, or import a{" "}
        <code className="font-mono text-[11.5px]">claude_desktop_config.json</code>. A name that
        already exists is replaced.
      </p>

      <textarea
        value={json}
        onChange={(e) => setJson(e.target.value)}
        spellCheck={false}
        rows={5}
        placeholder={`{
  "mcpServers": {
    "notes": { "command": "bunx", "args": ["-y", "mcp-server-notes"] }
  }
}`}
        className="clay-field mt-3"
      />

      <div className="mt-3 flex flex-wrap items-center gap-2">
        <Button primary={primary} disabled={busy} onClick={() => void submit()}>
          {busy ? "Spawning…" : "Add and start"}
        </Button>
        <label className="clay-button cursor-pointer">
          Import file
          <input
            type="file"
            accept="application/json,.json"
            hidden
            onChange={(e) => {
              const file = e.target.files?.[0];
              e.target.value = "";
              if (!file) return;
              void file
                .text()
                .then((text) => {
                  setJson(text);
                  setError("");
                })
                .catch((err: unknown) => setError(`cannot read file: ${String(err)}`));
            }}
          />
        </label>
        {error && (
          <span role="alert" className="font-mono text-[11.5px] break-words text-bad">
            {error}
          </span>
        )}
      </div>
    </Panel>
  );
}

export function Settings({
  tunnel,
  onTunnel,
  servers,
  onServers,
  approvalsDefault,
  registryPath,
  activity,
  running,
  flash,
  hubSessions,
}: {
  tunnel: TunnelInfo;
  onTunnel: (info: TunnelInfo) => void;
  servers: ServerRow[] | null;
  onServers: (rows: ServerRow[]) => void;
  approvalsDefault: string;
  registryPath: string;
  activity: ActivityEntry[];
  running: RunningCall[];
  flash: FlashState;
  hubSessions: number;
}) {
  const live = tunnel.status === "ready";

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-[19px] font-[600] tracking-[-0.018em]">Settings</h1>

      <Tunnel info={tunnel} onChange={onTunnel} primary={!live} />

      <ModelPanel />

      <Panel
        labelledBy="servers-title"
        title="Servers"
        right={
          <span className="text-[11.5px] text-dim tabular-nums">
            {servers?.length ?? 0} · {servers?.reduce((sum, s) => sum + s.tools.length, 0) ?? 0} tools
          </span>
        }
      >
        <Servers rows={servers} onServers={onServers} />
      </Panel>

      <Panel
        labelledBy="tools-title"
        title="Tools"
        right={
          <span className="text-[11.5px] text-dim tabular-nums">
            {hubSessions} sessions
          </span>
        }
      >
        <div className="grid grid-cols-1 gap-3 lg:grid-cols-[minmax(0,1fr)_360px]">
          <div className="clay-bump min-h-[440px] overflow-hidden !p-0">
            <ToolFlow servers={servers} flash={flash} running={running} sessions={hubSessions} />
          </div>
          <LiveRail running={running} activity={activity} bare />
        </div>
      </Panel>

      <AddServer onServers={onServers} primary={!live} />

      <Panel labelledBy="approvals-title" title="Approvals">
        <p className="m-0 text-[12.5px] text-dim">
          Every tool runs without asking. Default right now:{" "}
          <strong className="font-[550] text-ink">{approvalsDefault}</strong>.
        </p>
        <p className="mt-2 mb-0 text-[12.5px] text-dim">
          Web chats time out long before you can tap Approve, so a prompt looks like a broken
          bridge. Set a tool to <strong className="font-[550] text-ink">Ask</strong> in Servers only
          when you want that brake, and it lands on Overview.{" "}
          <strong className="font-[550] text-ink">Deny</strong> stops a tool outright. The real kill
          switch is Revoke link, above.
        </p>
      </Panel>

      <Panel labelledBy="paths-title" title="Files">
        <dl className="m-0 flex flex-col gap-2 text-[11.5px]">
          <div className="flex gap-2">
            <dt className="w-[74px] shrink-0 text-dim">Registry</dt>
            <dd className="m-0 min-w-0 truncate font-mono text-sub">{registryPath || "—"}</dd>
          </div>
          <div className="flex gap-2">
            <dt className="w-[74px] shrink-0 text-dim">Skills</dt>
            <dd className="m-0 min-w-0 truncate font-mono text-sub">
              drop a folder with SKILL.md into the registry's .agents/skills
            </dd>
          </div>
        </dl>
      </Panel>
    </div>
  );
}
