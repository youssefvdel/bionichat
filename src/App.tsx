import { useCallback, useEffect, useRef, useState } from "react";
import type { ActivityEntry, CallStart, Dashboard, Pending, ServerRow, TunnelInfo } from "./types";
import { invoke, bridge } from "./tauri";
import { Titlebar } from "./components/Titlebar";
import { Sidebar, type View } from "./components/Sidebar";
import { Overview } from "./views/Overview";
import { Projects } from "./views/Projects";
import { Settings } from "./views/Settings";
import type { FlashState, RunningCall } from "./components/ToolFlow";

export default function App() {
  const [view, setView] = useState<View>("overview");
  const [renderer, setRenderer] = useState("—");
  const [version, setVersion] = useState("");
  const [hubSessions, setHubSessions] = useState(0);
  const [tunnel, setTunnel] = useState<TunnelInfo>({ status: "off", public_url: null, error: null, kind: null });
  const [approvals, setApprovals] = useState<Pending[]>([]);
  const [approvalsDefault, setApprovalsDefault] = useState("ask");
  const [servers, setServers] = useState<ServerRow[] | null>(null);
  const [registryPath, setRegistryPath] = useState("—");
  const [activity, setActivity] = useState<ActivityEntry[]>([]);
  const [running, setRunning] = useState<RunningCall[]>([]);
  const [flash, setFlash] = useState<FlashState>({});
  const [loadError, setLoadError] = useState("");
  const activityRef = useRef<ActivityEntry[]>([]);
  activityRef.current = activity;

  const applyDashboard = useCallback((data: Dashboard) => {
    setRenderer(data.renderer);
    setVersion(data.version);
    setApprovalsDefault(data.approvals_default);
    setRegistryPath(data.registry_path);
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
        .listen<CallStart>("tool-start", (e) => {
          if (cancelled) return;
          const call = e.payload;
          setRunning((prev) => [
            ...prev,
            {
              key: `${call.server}::${call.tool}`,
              server: call.server,
              tool: call.tool,
              arguments: call.arguments,
              at: call.at_ms,
            },
          ]);
        })
        .then((off) => unlistens.push(off));
      void b.event
        .listen<ActivityEntry>("tool-activity", (e) => {
          if (cancelled) return;
          const entry = e.payload;
          setActivity([entry, ...activityRef.current].slice(0, 50));
          const key = `${entry.server}::${entry.tool}`;
          const mark = entry.status === "ok" ? "ok" : "error";
          setFlash((prev) => ({ ...prev, [key]: mark }));
          window.setTimeout(() => {
            setFlash((prev) => {
              if (prev[key] !== mark) return prev;
              const next = { ...prev };
              delete next[key];
              return next;
            });
          }, 6000);
          setRunning((prev) => {
            const key = `${entry.server}::${entry.tool}`;
            const index = prev.findIndex((call) => call.key === key);
            if (index === -1) return prev;
            return [...prev.slice(0, index), ...prev.slice(index + 1)];
          });
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
          version={version}
        />
        <main className="min-h-0 flex-1 overflow-y-auto pr-5 pb-8">
          {loadError && (
            <p className="mb-4 rounded-md bg-bad/12 px-3 py-2 font-mono text-[11.5px] text-bad">
              {loadError}
            </p>
          )}
          {view === "overview" && (
            <Overview
              tunnel={tunnel}
              approvals={approvals}
              onApprovals={setApprovals}
              servers={servers}
              flash={flash}
              running={running}
              activity={activity}
              sessions={hubSessions}
              onOpenSettings={() => setView("settings")}
              onOpenProjects={() => setView("projects")}
            />
          )}
          {view === "projects" && <Projects />}
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
