import type { Pending, TunnelInfo } from "../types";
import { Approvals } from "../components/Approvals";
import { Button, Panel } from "../components/ui";
import { Pill } from "../components/chrome";

export function Overview({
  tunnel,
  approvals,
  onApprovals,
  onOpenSettings,
  onOpenProjects,
}: {
  tunnel: TunnelInfo;
  approvals: Pending[];
  onApprovals: (list: Pending[]) => void;
  onOpenSettings: () => void;
  onOpenProjects: () => void;
}) {
  const live = tunnel.status === "ready";

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-[19px] font-[600] tracking-[-0.018em]">Overview</h1>

      <Approvals list={approvals} onChange={onApprovals} />

      {/* The public link is a key to this machine: it is shown, and copied,
          only inside Settings → Tunnel. */}
      <Panel>
        <div className="flex flex-wrap items-center gap-3">
          <Pill status={live ? "ready" : tunnel.status} />
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="text-[13px] font-[550]">Public link</span>
            <span className="text-[12px] text-dim">
              {live
                ? "Live. Anyone holding it can call your tools, so keep it private."
                : "Off. Connections cannot reach this PC until you start the tunnel."}
            </span>
          </div>
          <span className="ml-auto">
            <Button onClick={onOpenSettings}>
              {live ? "Manage link" : "Set up the tunnel"}
            </Button>
          </span>
        </div>
      </Panel>

      <Panel>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex flex-col gap-0.5">
            <span className="text-[13px] font-[550]">Pick a project</span>
            <span className="text-[12px] text-dim">
              Your folders in ~/Projects. A chat opens here next.
            </span>
          </div>
          <Button onClick={onOpenProjects}>Open projects</Button>
        </div>
      </Panel>

      <p className="m-0 px-1 text-[11.5px] text-dim">
        The link, servers, tool policies and the live map are in Settings.
      </p>
    </div>
  );
}
