import { memo, useMemo } from "react";
import {
  Handle,
  Position,
  ReactFlow,
  ReactFlowProvider,
  type Edge,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type { ActivityEntry, ServerRow } from "../types";
import { Mark } from "./chrome";

export type FlashState = Record<string, "ok" | "error">;

export interface RunningCall {
  key: string;
  server: string;
  tool: string;
  arguments: Record<string, unknown> | null;
  at: number;
}

type ToolState = "idle" | "running" | "ok" | "error";
type ToolData = { label: string; readOnly: boolean; state: ToolState; hint?: string };
type ServerData = { label: string; status: string; tools: number };
type HubData = { sessions: number; busy: boolean };

const stateRing: Record<ToolState, string> = {
  idle: "",
  running: "!ring-2 !ring-accent/70 !shadow-[0_0_0_6px_rgba(0,122,255,0.10)]",
  ok: "!ring-2 !ring-[#b7e2c6]",
  error: "!ring-2 !ring-[#f0bdb9]",
};

const ToolNode = memo(({ data }: NodeProps) => {
  const d = data as unknown as ToolData;
  return (
    <div
      title={`${d.label}${d.hint && d.hint !== d.label ? ` — ${d.hint}` : ""}`}
      className={`clay-bump flex h-[38px] w-[132px] items-center gap-1.5 px-3 transition-all duration-300 ${stateRing[d.state]}`}
    >
      <Handle type="target" position={Position.Top} className="tf-handle" />
      <span className="truncate font-mono text-[11.5px] font-[550] text-ink">{d.label}</span>
      <span className="ml-auto flex shrink-0 items-center gap-1">
        {d.readOnly && <span className="font-mono text-[9px] text-dim">RO</span>}
        {d.state !== "idle" && (
          <span
            className={`h-1.5 w-1.5 rounded-full ${
              d.state === "running" ? "animate-pulse bg-accent" : d.state === "ok" ? "bg-good" : "bg-bad"
            }`}
          />
        )}
      </span>
    </div>
  );
});
ToolNode.displayName = "ToolNode";

const ServerNode = memo(({ data }: NodeProps) => {
  const d = data as unknown as ServerData;
  return (
    <div className="clay-panel flex h-[50px] w-[150px] items-center gap-2 px-3.5">
      <Handle type="target" position={Position.Left} className="tf-handle" />
      <span
        className={`h-2 w-2 shrink-0 rounded-full ${
          d.status === "ready" ? "bg-good" : d.status === "error" ? "bg-bad" : "bg-[#c9c9d0]"
        }`}
      />
      <span className="truncate font-mono text-[12px] font-[600] text-ink">{d.label}</span>
      <span className="ml-auto shrink-0 text-[10px] text-dim tabular-nums">{d.tools}</span>
      <Handle type="source" position={Position.Bottom} className="tf-handle" />
    </div>
  );
});
ServerNode.displayName = "ServerNode";

// Hub to server runs down a lane between columns, like a patch panel.
const BusEdge = ({
  sourceX,
  sourceY,
  targetX,
  targetY,
  data,
}: {
  sourceX: number;
  sourceY: number;
  targetX: number;
  targetY: number;
  data?: Record<string, unknown>;
}) => {
  const lane = typeof data?.lane === "number" ? data.lane : targetX - 8;
  const drop = sourceY + 14;
  const d = `M ${sourceX},${sourceY} L ${sourceX},${drop} L ${lane},${drop} L ${lane},${targetY} L ${targetX},${targetY}`;
  return <path d={d} className="react-flow__edge-path" fill="none" />;
};

const edgeTypes = { bus: BusEdge };

const HubNode = memo(({ data }: NodeProps) => {
  const d = data as unknown as HubData;
  return (
    <div className="clay-panel flex h-[68px] w-[168px] items-center gap-2.5 px-4">
      <span className={d.busy ? "text-ink" : "text-dim"}>
        <Mark size={19} />
      </span>
      <div className="flex min-w-0 flex-col leading-tight">
        <span className="text-[12px] font-[600]">hub</span>
        <span className="text-[10px] text-dim tabular-nums">{d.sessions} sessions</span>
      </div>
      <Handle type="source" position={Position.Bottom} className="tf-handle" />
    </div>
  );
});
HubNode.displayName = "HubNode";

const nodeTypes = { tool: ToolNode, server: ServerNode, hub: HubNode };

const COL = 164; // one server column, edge to edge
const X0 = 26; // left margin, room for the bus lanes
const SERVER_W = 150;
const TOOL_W = 132;
const TOOL_PITCH = 44;
const SERVER_Y = 92;
const TOOL_Y = 156;

function Flow({
  servers,
  flash,
  running,
  sessions,
}: {
  servers: ServerRow[] | null;
  flash: Record<string, ToolState>;
  running: RunningCall[];
  sessions: number;
}) {
  const { nodes, edges, signature } = useMemo(() => {
    const list = servers ?? [];
    const runningKeys = new Set(running.map((r) => r.key));

    const outNodes: Node[] = [];
    const outEdges: Edge[] = [];

    // Servers wrap into rows of three so nodes stay near full size.
    const perRow = Math.min(3, Math.max(list.length, 1));
    const rows: typeof list[] = [];
    for (let i = 0; i < list.length; i += perRow) rows.push(list.slice(i, i + perRow));

    const rowHeights = rows.map(
      (row) => 50 + Math.max(...row.map((s) => s.tools.length), 1) * TOOL_PITCH + 26,
    );
    const width = Math.min(rows.length ? Math.max(...rows.map((r) => r.length)) : 1, 3) * COL;

    let rowY = 96;
    rows.forEach((row, r) => {
      row.forEach((server, col) => {
        const colX = X0 + ((perRow - row.length) * COL) / 2 + col * COL;
        const toolX = colX + (SERVER_W - TOOL_W) / 2;
        server.tools.forEach((tool, i) => {
          const key = `${server.name}::${tool.name}`;
          const isRunning = runningKeys.has(key);
          const state: ToolState = isRunning ? "running" : (flash[key] ?? "idle");
          outNodes.push({
            id: `tool-${key}`,
            type: "tool",
            position: { x: toolX, y: rowY + 78 + i * TOOL_PITCH },
            data: {
            label: `${server.namespace}_${tool.name}`,
            readOnly: tool.read_only,
            state,
            hint: tool.description ?? tool.name,
          } satisfies ToolData,
          });
          outEdges.push({
            id: `e-${key}`,
            source: `srv-${server.name}`,
            target: `tool-${key}`,
            animated: isRunning,
            className: isRunning ? "tf-edge tf-edge-live" : "tf-edge",
          });
        });

        outNodes.push({
          id: `srv-${server.name}`,
          type: "server",
          position: { x: colX, y: rowY },
          data: { label: server.name, status: server.status, tools: server.tools.length } satisfies ServerData,
        });
        const busy = server.tools.some((t) => runningKeys.has(`${server.name}::${t.name}`));
        outEdges.push({
          id: `e-hub-${server.name}`,
          source: "hub",
          target: `srv-${server.name}`,
          type: "bus",
          data: { lane: colX - 13 },
          animated: busy,
          className: busy ? "tf-edge tf-edge-live" : "tf-edge",
        });
      });
      rowY += rowHeights[r] ?? 0;
    });

    outNodes.push({
      id: "hub",
      type: "hub",
      position: { x: X0 + Math.max((width - 168) / 2, 0), y: 0 },
      data: { sessions, busy: running.length > 0 } satisfies HubData,
    });

    const sig = JSON.stringify([
      list.map((s) => [s.name, s.namespace, s.status, s.tools.map((t) => t.name)]),
      running.map((r) => r.key).sort(),
    ]);

    return { nodes: outNodes, edges: outEdges, signature: sig };
  }, [servers, flash, running, sessions]);

  return (
    <div className="h-full w-full">
      <ReactFlow
      key={signature}
      nodes={nodes}
      edges={edges}
      fitView
      fitViewOptions={{ padding: 0.08, duration: 0 }}
      nodeTypes={nodeTypes}
      edgeTypes={edgeTypes}
      nodesDraggable={false}
      nodesConnectable={false}
      elementsSelectable={false}
      minZoom={0.4}
      maxZoom={1.4}
      proOptions={{ hideAttribution: false }}
      className="tf-canvas"
      style={{ background: "var(--color-surface)" }}
      >
      </ReactFlow>
    </div>
  );
}

export function ToolFlow(props: {
  servers: ServerRow[] | null;
  flash: Record<string, "ok" | "error">;
  running: RunningCall[];
  sessions: number;
}) {
  return (
    <ReactFlowProvider>
      <Flow {...props} />
    </ReactFlowProvider>
  );
}
