import { useCallback, useEffect, useRef, useState } from "react";
import type { ActivityEntry, Dashboard, Pending, ServerRow, TunnelInfo } from "./types";
import { invoke, bridge } from "./tauri";
import { Titlebar } from "./components/Titlebar";
import { TunnelCard } from "./components/TunnelCard";
import { ApprovalsCard } from "./components/ApprovalsCard";
import { ActivityCard } from "./components/ActivityCard";
import { ServersCard } from "./components/ServersCard";
import { AddCard } from "./components/AddCard";
import { ActionButton } from "./components/ui";
import { Pill } from "./components/chrome";

export default function App() {
  const [renderer, setRenderer] = useState("—");
  const [hubUrl, setHubUrl] = useState("—");
  const [hubSessions, setHubSessions] = useState("0 sessions");
  const [hubNote, setHubNote] = useState("JSON-RPC over POST, no SSE");
  const [tunnel, setTunnel] = useState<TunnelInfo>({ status: "off", public_url: null, error: null });
  const [approvals, setApprovals] = useState<Pending[]>([]);
  const [approvalsDefault, setApprovalsDefault] = useState("ask");
  const [servers, setServers] = useState<ServerRow[] | null>(null);
  const [registryPath, setRegistryPath] = useState("—");
  const [activity, setActivity] = useState<ActivityEntry[]>([]);
  const [refreshing, setRefreshing] = useState(false);
  const [loadError, setLoadError] = useState("");
  const activityRef = useRef<ActivityEntry[]>([]);
  activityRef.current = activity;

  const applyDashboard = useCallback((data: Dashboard) => {
    setRenderer(data.renderer);
    setApprovalsDefault(data.approvals_default);
    setRegistryPath(data.registry_path);
    setHubUrl(data.hub.url);
    setHubSessions(`${data.hub.sessions} session${data.hub.sessions === 1 ? "" : "s"}`);
    setHubNote(data.hub.note);
    setTunnel(data.tunnel);
    setApprovals(data.approvals);
    setServers(data.servers);
  }, []);

  useEffect(() => {
    let cancelled = false;
    invoke<Dashboard>("dashboard")
      .then((data) => {
        if (!cancelled) {
          applyDashboard(data);
          setLoadError("");
        }
      })
      .catch((e: unknown) => {
        if (!cancelled) setLoadError(`dashboard failed: ${String(e)}`);
      });
    const b = bridge();
    const unlistens: Array<() => void> = [];
    if (b) {
      void b.event
        .listen<ServerRow[]>("servers-updated", (event) => {
          if (!cancelled) setServers(event.payload);
        })
        .then((off) => unlistens.push(off));
      void b.event
        .listen<TunnelInfo>("tunnel-updated", (event) => {
          if (!cancelled) setTunnel(event.payload);
        })
        .then((off) => unlistens.push(off));
      void b.event.listen<Pending>("approval-requested", () => {
        if (cancelled) return;
        void invoke<Pending[]>("approvals")
          .then((list) => {
            if (!cancelled) setApprovals(list);
          })
          .catch(() => {});
      }).then((off) => unlistens.push(off));
      void b.event.listen<ActivityEntry>("tool-activity", (event) => {
        if (cancelled) return;
        setActivity([event.payload, ...activityRef.current].slice(0, 50));
      }).then((off) => unlistens.push(off));
      invoke<ActivityEntry[]>("activity")
        .then((list) => {
          if (!cancelled) setActivity(list.slice(0, 50));
        })
        .catch(() => {});
    }
    const timer = window.setInterval(() => {
      if (cancelled) return;
      invoke<Pending[]>("approvals")
        .then((list) => {
          if (!cancelled) setApprovals(list);
        })
        .catch(() => {});
    }, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      unlistens.forEach((off) => off());
    };
  }, [applyDashboard]);

  async function refresh() {
    setRefreshing(true);
    try {
      setServers(await invoke<ServerRow[]>("refresh"));
      setLoadError("");
    } catch (e) {
      setLoadError(`refresh failed: ${String(e)}`);
    } finally {
      setRefreshing(false);
    }
  }

  return (
    <div className="flex min-h-screen flex-col bg-ground text-ink">
      <Titlebar renderer={renderer} />
      <main className="mx-auto flex w-full max-w-[1080px] flex-col gap-5 px-7 pt-2 pb-10">
        {/* time-sensitive first: approvals above everything */}
        <ApprovalsCard list={approvals} onChange={setApprovals} />
        <TunnelCard info={tunnel} onChange={setTunnel} />

        <section aria-labelledby="hub-title" className="clay-panel px-5 pt-4 pb-5">
          <div className="mb-3 flex items-baseline justify-between gap-3">
            <h2 id="hub-title" className="text-[14px] font-[550] tracking-[-0.005em]">
              Hub
            </h2>
            <Pill status="idle" label={hubSessions} />
          </div>
          <div className="flex flex-wrap items-center gap-2.5">
            <code className="font-mono text-xs text-dim">{hubUrl}</code>
            <ActionButton
              onClick={() => {
                void navigator.clipboard.writeText(hubUrl).catch(() => {});
              }}
            >
              Copy URL
            </ActionButton>
            <span className="text-xs text-dim">{hubNote}</span>
          </div>
        </section>

        <ActivityCard feed={activity} />
        <ServersCard rows={servers} approvalsDefault={approvalsDefault} onServers={setServers} />
        <AddCard onServers={setServers} />
        {loadError && (
          <p className="m-0 font-mono text-xs text-bad">{loadError}</p>
        )}
      </main>
      <footer className="mx-auto flex w-full max-w-[1080px] items-center gap-2 px-7 py-3">
        <span className="text-xs text-dim">registry</span>
        <code className="overflow-hidden font-mono text-xs text-ellipsis whitespace-nowrap text-dim">
          {registryPath}
        </code>
      </footer>
    </div>
  );
}
