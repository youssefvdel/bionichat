import type { ActivityEntry, Pending, ServerRow, TunnelInfo } from "../types";
import { Approvals } from "../components/Approvals";
import { Button, Panel } from "../components/ui";
import { Pill } from "../components/chrome";
import { LiveRail } from "../components/LiveRail";
import { ToolFlow, type FlashState, type RunningCall } from "../components/ToolFlow";

export function Overview({
  tunnel,
  approvals,
  onApprovals,
  servers,
  flash,
  running,
  activity,
  sessions,
  onOpenSettings,
  onOpenProjects,
}: {
  tunnel: TunnelInfo;
  approvals: Pending[];
  onApprovals: (list: Pending[]) => void;
  servers: ServerRow[] | null;
  flash: FlashState;
  running: RunningCall[];
  activity: ActivityEntry[];
  sessions: number;
  onOpenSettings: () => void;
  onOpenProjects: () => void;
}) {
  const live = tunnel.status === "ready";
  const toolCount = servers?.reduce((sum, server) => sum + server.tools.length, 0) ?? 0;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="m-0 text-[19px] font-[600] tracking-[-0.018em]">Overview</h1>
        <span className="flex items-center gap-2">
          <Pill status={live ? "ready" : tunnel.status} label={live ? "link live" : "link off"} />
          <span className="font-mono text-[11px] text-dim tabular-nums">
            {servers?.length ?? 0} servers · {toolCount} tools · {sessions} sessions
          </span>
        </span>
      </div>

      <Approvals list={approvals} onChange={onApprovals} />

      <Panel
        labelledBy="map-title"
        title="Live map"
        right={
          <span className="font-mono text-[11px] text-dim tabular-nums">
            {running.length ? `${running.length} running` : "idle"}
          </span>
        }
      >
        <div className="grid grid-cols-1 gap-3 lg:grid-cols-[minmax(0,1fr)_360px]">
          <div className="clay-bump min-h-[520px] overflow-hidden !p-0">
            <ToolFlow servers={servers} flash={flash} running={running} sessions={sessions} />
          </div>
          <LiveRail running={running} activity={activity} bare />
        </div>
      </Panel>

      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
        {/* The public link is a key to this machine: it is shown, and copied,
            only inside Settings → Tunnel. */}
        <Panel>
          <div className="flex items-center gap-3">
            <Pill status={live ? "ready" : tunnel.status} />
            <div className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span className="text-[13px] font-[550]">Public link</span>
              <span className="truncate text-[12px] text-dim">
                {live ? "Live. Anyone with it can call your tools." : "Off. Nothing can reach this PC."}
              </span>
            </div>
            <Button onClick={onOpenSettings}>{live ? "Manage link" : "Set up"}</Button>
          </div>
        </Panel>

        <Panel>
          <div className="flex items-center gap-3">
            <div className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span className="text-[13px] font-[550]">Projects</span>
              <span className="truncate text-[12px] text-dim">Folders a chat can work in.</span>
            </div>
            <Button onClick={onOpenProjects}>Open</Button>
          </div>
        </Panel>
      </div>
    </div>
  );
}
