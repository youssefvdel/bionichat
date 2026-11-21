import { useState } from "react";
import type { ActivityEntry, Pending, ServerRow, TunnelInfo } from "../types";
import { Approvals } from "../components/Approvals";
import { Activity } from "../components/Activity";
import { Button, Panel } from "../components/ui";
import { Pill } from "../components/chrome";

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const helper = document.createElement("textarea");
    helper.value = text;
    helper.style.position = "fixed";
    helper.style.opacity = "0";
    document.body.append(helper);
    helper.select();
    const ok = document.execCommand("copy");
    helper.remove();
    return ok;
  }
}

function Count({ value, label }: { value: number | string; label: string }) {
  return (
    <div className="flex flex-col gap-1 px-5 py-4 first:pl-0 last:pr-0">
      <span className="font-mono text-[22px] leading-none tabular-nums">{value}</span>
      <span className="text-[11.5px] text-dim">{label}</span>
    </div>
  );
}

export function Overview({
  tunnel,
  hubUrl,
  hubSessions,
  servers,
  activity,
  approvals,
  onApprovals,
  onOpenSettings,
}: {
  tunnel: TunnelInfo;
  hubUrl: string;
  hubSessions: number;
  servers: ServerRow[] | null;
  activity: ActivityEntry[];
  approvals: Pending[];
  onApprovals: (list: Pending[]) => void;
  onOpenSettings: () => void;
}) {
  const [copied, setCopied] = useState("");
  const live = tunnel.status === "ready";
  const ready = servers?.filter((s) => s.status === "ready").length ?? 0;
  const tools = servers?.reduce((sum, s) => sum + s.tools.length, 0) ?? 0;

  function copy() {
    if (!tunnel.public_url) return;
    void copyText(tunnel.public_url).then((ok) => {
      setCopied(ok ? "Copied" : "Copy failed");
      window.setTimeout(() => setCopied(""), 1400);
    });
  }

  return (
    <div className="flex flex-col gap-5">
      <div className="flex items-center justify-between gap-3">
        <h1 className="text-[19px] font-[600] tracking-[-0.018em]">Overview</h1>
      </div>
      <Approvals list={approvals} onChange={onApprovals} />

      <Panel>
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <span className="text-[11.5px] text-dim">Your chat reaches this PC at</span>
            <code className="mt-1 block truncate font-mono text-[17px] leading-snug text-ink">
              {tunnel.public_url ?? "no public link yet"}
            </code>
          </div>
          <Pill status={live ? "ready" : tunnel.status} />
        </div>

        <div className="mt-3.5 flex flex-wrap items-center gap-2">
          {live ? (
            <Button primary onClick={copy}>
              {copied || "Copy link"}
            </Button>
          ) : (
            <Button primary onClick={onOpenSettings}>
              Set up the tunnel
            </Button>
          )}
          <span className="text-[11.5px] text-dim">
            {live
              ? "Paste it into Gemini, ChatGPT or Claude as a custom connector."
              : "Settings → Tunnel starts a public link for this machine."}
          </span>
        </div>
      </Panel>

      <Panel className="!px-5 !py-0">
        <div className="flex divide-x divide-rule">
          <Count value={servers?.length ?? 0} label="projects" />
          <Count value={ready} label="running" />
          <Count value={tools} label="tools exposed" />
          <Count value={hubSessions} label="sessions" />
        </div>
      </Panel>

      <Activity feed={activity} />

      <p className="m-0 px-1 text-[11.5px] text-dim">
        Local endpoint <code className="font-mono text-[11.5px]">{hubUrl}</code> · JSON-RPC over
        POST
      </p>
    </div>
  );
}
