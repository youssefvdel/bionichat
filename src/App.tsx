import { useCallback, useEffect, useRef, useState } from "react";
import type { ActivityEntry, Dashboard, Pending, ServerRow, TunnelInfo } from "./types";
import { invoke, bridge } from "./tauri";
import { Titlebar } from "./components/Titlebar";
import { Sidebar, type View } from "./components/Sidebar";
import { Overview } from "./views/Overview";
import { Projects } from "./views/Projects";
import { Settings } from "./views/Settings";

export default function App() {
  const [view, setView] = useState<View>("overview");
  const [renderer, setRenderer] = useState("—");
  const [hubUrl, setHubUrl] = useState("—");
  const [hubSessions, setHubSessions] = useState(0);
  const [tunnel, setTunnel] = useState<TunnelInfo>({ status: "off", public_url: null, error: null });
  const [approvals, setApprovals] = useState<Pending[]>([]);
  const [approvalsDefault, setApprovalsDefault] = useState("ask");
  const [servers, setServers] = useState<ServerRow[] | null>(null);
  const [registryPath, setRegistryPath] = useState("—");
  const [activity, setActivity] = useState<ActivityEntry[]>([]);
  const [loadError, setLoadError] = useState("");
  const activityRef = useRef<ActivityEntry[]>([]);
  activityRef.current = activity;

  const applyDashboard = useCallback((data: Dashboard) => {
    setRenderer(data.renderer);
    setApprovalsDefault(data.approvals_default);
    setRegistryPath(data.registry_path);
    setHubUrl(data.hub.url);
    setHubSessions(data.hub.sessions);
    setTunnel(data.tunnel);
    setApprovals(data.approvals);
    setServers(data.servers);
  }, []);

  useEffect(() => {
    let cancelled = false;
    invoke<Dashboard>("dashboard")
      .then((data) => {
        if (cancelled) return;
        applyDashboard(data);
        setLoadError("");
      })
      .catch((e: unknown) => {
        if (!cancelled) setLoadError(`Could not read the dashboard: ${String(e)}`);
      });

    const b = bridge();
    const unlistens: Array<() => void> = [];
    if (b) {
      void b.event
        .listen<ServerRow[]>("servers-updated", (e) => {
          if (!cancelled) setServers(e.payload);
        })
        .then((off) => unlistens.push(off));
      void b.event
        .listen<TunnelInfo>("tunnel-updated", (e) => {
          if (!cancelled) setTunnel(e.payload);
        })
        .then((off) => unlistens.push(off));
      void b.event
        .listen<Pending>("approval-requested", () => {
          if (cancelled) return;
          void invoke<Pending[]>("approvals")
            .then((list) => {
              if (!cancelled) setApprovals(list);
            })
            .catch(() => {});
        })
        .then((off) => unlistens.push(off));
      void b.event
        .listen<ActivityEntry>("tool-activity", (e) => {
          if (cancelled) return;
          setActivity([e.payload, ...activityRef.current].slice(0, 50));
        })
        .then((off) => unlistens.push(off));
      void invoke<ActivityEntry[]>("activity")
        .then((list) => {
          if (!cancelled) setActivity(list.slice(0, 50));
        })
        .catch(() => {});
    }

    const timer = window.setInterval(() => {
      if (cancelled) return;
      void invoke<Pending[]>("approvals")
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

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-ground text-ink">
      <Titlebar renderer={renderer} />
      <div className="flex min-h-0 flex-1">
        <Sidebar
          view={view}
          onView={setView}
          waiting={approvals.length}
        />
        <main className="min-h-0 flex-1 overflow-y-auto pr-5 pb-8">
          {loadError && (
            <p className="mb-4 rounded-md bg-[#fdeceb] px-3 py-2 font-mono text-[11.5px] text-bad">
              {loadError}
            </p>
          )}
          {view === "overview" && (
            <Overview
              tunnel={tunnel}
              hubUrl={hubUrl}
              hubSessions={hubSessions}
              servers={servers}
              activity={activity}
              approvals={approvals}
              onApprovals={setApprovals}
              onOpenSettings={() => setView("settings")}
            />
          )}
          {view === "projects" && (
            <Projects
              rows={servers}
              onServers={setServers}
              onAdd={() => setView("settings")}
            />
          )}
          {view === "settings" && (
            <Settings
              tunnel={tunnel}
              onTunnel={setTunnel}
              servers={servers}
              onServers={setServers}
              approvalsDefault={approvalsDefault}
              registryPath={registryPath}
            />
          )}
        </main>
      </div>
    </div>
  );
}
