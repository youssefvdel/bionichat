export type Status = "pending" | "ready" | "error";
export type Decision = "allow" | "ask" | "deny";

export interface ToolInfo {
  name: string;
  description: string | null;
  read_only: boolean;
  input_schema: Record<string, unknown>;
  policy: Decision;
}

export interface ServerRow {
  name: string;
  namespace: string;
  builtin: boolean;
  transport: string;
  secret_keys: string[];
  autorestart: boolean;
  status: Status;
  error: string | null;
  tools: ToolInfo[];
}

export interface Pending {
  id: number;
  server: string;
  tool: string;
  arguments: Record<string, unknown> | null;
}

export interface ActivityEntry {
  time: string;
  server: string;
  tool: string;
  arguments: Record<string, unknown> | null;
  status: "ok" | "denied" | "timeout" | "error";
  result: string | null;
  duration_ms: number;
}

export interface ProjectEntry {
  name: string;
  path: string;
}

export interface DirRow {
  name: string;
  path: string;
}

export interface BrowseView {
  path: string;
  parent: string | null;
  dirs: DirRow[];
}

export interface CallStart {
  server: string;
  tool: string;
  arguments: Record<string, unknown> | null;
  at_ms: number;
}

export interface TunnelInfo {
  status: "off" | "starting" | "ready" | "error";
  public_url: string | null;
  error: string | null;
}

export interface HubRow {
  url: string;
  port: number;
  sessions: number;
  note: string;
}

export interface Dashboard {
  version: string;
  servers: ServerRow[];
  hub: HubRow;
  tunnel: TunnelInfo;
  approvals: Pending[];
  approvals_default: string;
  renderer: string;
  registry_path: string;
}
