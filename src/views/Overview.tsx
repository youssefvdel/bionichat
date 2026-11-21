import { useState } from "react";
import type { Pending, TunnelInfo } from "../types";
import { Approvals } from "../components/Approvals";
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
  const [copied, setCopied] = useState("");
  const live = tunnel.status === "ready";

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-[19px] font-[600] tracking-[-0.018em]">Overview</h1>

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
            <Button
              primary
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
        Tools, servers and the live map are in Settings.
      </p>
    </div>
  );
}
