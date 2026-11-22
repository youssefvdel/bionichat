import { useEffect, useRef, useState } from "react";
import type { ChatTurn, ModelView, ProjectEntry } from "../types";
import { bridge, invoke } from "../tauri";
import { Button, Panel } from "../components/ui";
import { CallPreview } from "../components/CallPreview";

type Live = { name: string; arguments: Record<string, unknown>; done: boolean };

export function Chat({
  project,
  onOpenSettings,
}: {
  project: ProjectEntry;
  onOpenSettings: () => void;
}) {
  const [turns, setTurns] = useState<ChatTurn[]>([]);
  const [prompt, setPrompt] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [model, setModel] = useState<ModelView | null>(null);
  const [live, setLive] = useState<Live[]>([]);
  const endRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    invoke<ModelView>("model_config")
      .then(setModel)
      .catch(() => setModel(null));
  }, []);

  useEffect(() => {
    let cancelled = false;
    const off: Array<() => void> = [];
    const bus = bridge();
    if (!bus?.event) return;
    void bus.event
      .listen<{ name: string; arguments: Record<string, unknown> }>("chat-tool-start", (e) => {
        if (cancelled) return;
        setLive((prev) => [...prev, { ...e.payload, done: false }]);
      })
      .then((un) => off.push(un));
    return () => {
      cancelled = true;
      off.forEach((un) => un());
    };
  }, []);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [turns, busy, live]);

  async function send() {
    const text = prompt.trim();
    if (!text || busy) return;
    if (model && !model.configured) {
      setError("Add a model key in Settings first.");
      return;
    }
    setError("");
    const history = turns;
    setTurns([...history, { role: "user", content: text, calls: [] }]);
    setPrompt("");
    setBusy(true);
    setLive([]);
    try {
      const reply = await invoke<ChatTurn>("chat_send", {
        project: project.path,
        history,
        prompt: text,
      });
      setTurns((prev) => [...prev, reply]);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      setLive([]);
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex items-baseline justify-between gap-3">
        <div className="flex min-w-0 flex-col">
          <h1 className="truncate text-[19px] font-[600] tracking-[-0.018em]">{project.name}</h1>
          <span className="truncate font-mono text-[10.5px] text-dim">{project.path}</span>
        </div>
        <span className="shrink-0 text-[11.5px] text-dim">
          {model?.configured ? model.model : "no model key"}
        </span>
      </div>

      <div className="clay-panel flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4">
          {turns.length === 0 && (
            <div className="flex h-full flex-col items-start justify-center gap-2">
              <p className="m-0 max-w-[52ch] text-[13px] text-sub">
                This chat runs on this machine and can use your tools in{" "}
                <span className="font-mono text-[12px]">{project.name}</span>.
              </p>
              <p className="m-0 max-w-[52ch] text-[12px] text-dim">
                Ask for something concrete: “list the files here”, “what does this repo do?”,
                “run the tests”.
              </p>
            </div>
          )}

          <ul className="m-0 flex list-none flex-col gap-3 p-0">
            {turns.map((turn, i) => (
              <li key={i} className="flex flex-col gap-2">
                {turn.role === "user" ? (
                  <div className="ml-auto max-w-[76%] rounded-2xl bg-ink px-3.5 py-2 text-[12.5px] text-white">
                    {turn.content}
                  </div>
                ) : (
                  <>
                    {turn.calls.map((call, j) => (
                      <CallPreview
                        key={j}
                        server=""
                        tool={call.name}
                        args={call.arguments}
                        result={call.result}
                        status={call.ok ? "ok" : "error"}
                      />
                    ))}
                    {turn.content && (
                      <p className="m-0 max-w-[76ch] text-[12.5px] leading-[1.6] whitespace-pre-wrap text-ink">
                        {turn.content}
                      </p>
                    )}
                  </>
                )}
              </li>
            ))}

            {busy && (
              <li className="flex flex-col gap-2">
                {live.map((call, i) => (
                  <CallPreview key={i} server="" tool={call.name} args={call.arguments} result={null} running />
                ))}
                <span className="text-[12px] text-dim">thinking…</span>
              </li>
            )}
          </ul>
          <div ref={endRef} />
        </div>

        {error && (
          <p role="alert" className="m-0 border-t border-rule px-4 py-2 font-mono text-[11.5px] text-bad">
            {error}
            {model && !model.configured && (
              <button
                type="button"
                onClick={onOpenSettings}
                className="ml-2 underline underline-offset-2"
              >
                open Settings
              </button>
            )}
          </p>
        )}

        <div className="border-t border-rule p-3">
          <div className="flex items-end gap-2">
            <textarea
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  void send();
                }
              }}
              rows={2}
              placeholder={`Ask ${project.name}…`}
              className="clay-field max-h-[140px] flex-1 resize-none"
            />
            <Button primary disabled={busy || !prompt.trim()} onClick={() => void send()}>
              {busy ? "…" : "Send"}
            </Button>
          </div>
        </div>
      </div>

      {!model?.configured && (
        <Panel>
          <p className="m-0 text-[12px] text-dim">
            A chat needs a model. Point it at any OpenAI-compatible endpoint in Settings → Model.
          </p>
        </Panel>
      )}
    </div>
  );
}
