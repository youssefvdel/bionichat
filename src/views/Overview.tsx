import { useState } from "react";
import type { ActivityEntry, Pending, ServerRow, TunnelInfo } from "../types";
import { Approvals } from "../components/Approvals";
import { LiveRail } from "../components/LiveRail";
import { ToolFlow, type FlashState, type RunningCall } from "../components/ToolFlow";
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

export function Overview({
  tunnel,
  servers,
  activity,
  running,
  flash,
  approvals,
  hubSessions,
  onApprovals,
  onOpenSettings,
}: {
  tunnel: TunnelInfo;
  servers: ServerRow[] | null;
  activity: ActivityEntry[];
  running: RunningCall[];
  flash: FlashState;
  approvals: Pending[];
  hubSessions: number;
  onApprovals: (list: Pending[]) => void;
  onOpenSettings: () => void;
}) {
  const [copied, setCopied] = useState("");
  const live = tunnel.status === "ready";
  const toolCount = servers?.reduce((sum, s) => sum + s.tools.length, 0) ?? 0;

  return (
    <div className="flex min-h-0 flex-col gap-4">
      <div className="flex items-center justify-between gap-3">
        <h1 className="text-[19px] font-[600] tracking-[-0.018em]">Overview</h1>
        <span className="text-[11.5px] text-dim tabular-nums">
          {servers?.length ?? 0} projects · {toolCount} tools · {hubSessions} sessions
        </span>
      </div>

      <Approvals list={approvals} onChange={onApprovals} />

      <Panel className="!py-3.5">
        <div className="flex flex-wrap items-center gap-3">
          <Pill status={live ? "ready" : tunnel.status} />
          <code className="min-w-0 flex-1 truncate font-mono text-[13px] text-ink">
            {tunnel.public_url ?? "no public link yet"}
          </code>
          {live ? (
            <Button
              primary
              small
              onClick={() => {
                void copyText(tunnel.public_url ?? "").then((ok) => {
                  setCopied(ok ? "Copied" : "Copy failed");
                  window.setTimeout(() => setCopied(""), 1400);
                });
              }}
            >
              {copied || "Copy link"}
            </Button>
          ) : (
            <Button primary small onClick={onOpenSettings}>
              Set up the tunnel
            </Button>
          )}
        </div>
      </Panel>

      <div className="grid min-h-[440px] flex-1 grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_310px]">
        <div className="clay-panel relative min-h-[400px] overflow-hidden">
          <ToolFlow
            servers={servers}
            flash={flash}
            running={running}
            sessions={hubSessions}
          />
        </div>
        <LiveRail running={running} activity={activity} />
      </div>
    </div>
  );
}
