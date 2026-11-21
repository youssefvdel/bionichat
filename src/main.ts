// Dashboard for bionichat pass 1: registry-backed MCP server list.
// No framework, no npm deps — bun compiles this to dist/main.js.

type Status = "pending" | "ready" | "error";
type Decision = "allow" | "ask" | "deny";

interface ToolInfo {
  name: string;
  description: string | null;
  read_only: boolean;
  input_schema: Record<string, unknown>;
  policy: Decision;
}

interface ServerRow {
  name: string;
  transport: string;
  secret_keys: string[];
  autorestart: boolean;
  status: Status;
  error: string | null;
  tools: ToolInfo[];
}

interface Pending {
  id: number;
  server: string;
  tool: string;
  arguments: Record<string, unknown> | null;
}

interface TunnelInfo {
  status: "off" | "starting" | "ready" | "error";
  public_url: string | null;
  error: string | null;
}

interface HubRow {
  url: string;
  port: number;
  sessions: number;
  note: string;
}

interface Dashboard {
  servers: ServerRow[];
  hub: HubRow;
  tunnel: TunnelInfo;
  approvals: Pending[];
  approvals_default: string;
  renderer: string;
  registry_path: string;
}

interface TauriGlobal {
  core: {
    invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  };
  event: {
    listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<() => void>;
  };
}

declare global {
  interface Window {
    __TAURI__?: TauriGlobal;
  }
}

export {};

const bridge = window.__TAURI__;

function qs<T extends Element>(selector: string): T {
  const node = document.querySelector<T>(selector);
  if (!node) throw new Error(`missing element: ${selector}`);
  return node;
}

const refs = {
  renderer: qs<HTMLSpanElement>("#renderer-chip"),
  refresh: qs<HTMLButtonElement>("#refresh"),
  hubUrl: qs<HTMLElement>("#hub-url"),
  hubCopy: qs<HTMLButtonElement>("#hub-copy"),
  hubSessions: qs<HTMLSpanElement>("#hub-sessions"),
  hubNote: qs<HTMLElement>("#hub-note"),
  tunnelStatus: qs<HTMLSpanElement>("#tunnel-status"),
  tunnelNote: qs<HTMLParagraphElement>("#tunnel-note"),
  tunnelUrl: qs<HTMLElement>("#tunnel-url"),
  tunnelCopy: qs<HTMLButtonElement>("#tunnel-copy"),
  tunnelStart: qs<HTMLButtonElement>("#tunnel-start"),
  tunnelStop: qs<HTMLButtonElement>("#tunnel-stop"),
  tunnelError: qs<HTMLParagraphElement>("#tunnel-error"),
  serverCount: qs<HTMLSpanElement>("#server-count"),
  servers: qs<HTMLUListElement>("#servers"),
  serversEmpty: qs<HTMLParagraphElement>("#servers-empty"),
  approvals: qs<HTMLElement>("#approvals"),
  approvalsCard: qs<HTMLElement>("#approvals-card"),
  approvalsList: qs<HTMLUListElement>("#approvals-list"),
  approvalsCount: qs<HTMLSpanElement>("#approvals-count"),
  addJson: qs<HTMLTextAreaElement>("#add-json"),
  addSubmit: qs<HTMLButtonElement>("#add-submit"),
  addFile: qs<HTMLInputElement>("#add-file"),
  addError: qs<HTMLSpanElement>("#add-error"),
  registryPath: qs<HTMLElement>("#registry-path"),
};

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function statusPill(status: Status): HTMLSpanElement {
  const label = status === "ready" ? "ready" : status === "error" ? "error" : "starting";
  return el("span", `pill pill-${status}`, label);
}

const POLICY_LABELS: Array<[Decision, string]> = [
  ["allow", "Allow"],
  ["ask", "Ask"],
  ["deny", "Deny"],
];

function requiredList(tool: ToolInfo): string {
  const schema = tool.input_schema as { required?: unknown } | null;
  const required = Array.isArray(schema?.required) ? (schema.required as unknown[]) : [];
  const names = required.filter((name): name is string => typeof name === "string");
  return names.length > 0 ? ` — needs ${names.join(", ")}` : "";
}

function policySelect(row: ServerRow, tool: ToolInfo): HTMLSelectElement {
  const select = el("select", "policy");
  select.dataset.server = row.name;
  select.dataset.tool = tool.name;
  select.title = `Policy for ${row.name} / ${tool.name}`;
  for (const [value, label] of POLICY_LABELS) {
    const option = el("option", undefined, label);
    option.value = value;
    option.selected = tool.policy === value;
    select.append(option);
  }
  return select;
}

function serverNode(row: ServerRow): HTMLLIElement {
  const item = el("li", "server");

  const head = el("div", "server-head");
  head.append(el("span", "server-name", row.name));
  head.append(el("span", "server-transport", row.transport));

  if (row.secret_keys.length > 0) {
    const secrets = el("span", "secrets");
    for (const key of row.secret_keys) secrets.append(el("span", "secret", `${key} ••••`));
    head.append(secrets);
  }

  head.append(statusPill(row.status));

  const restart = el("button", "btn btn-ghost btn-small", "Restart");
  restart.type = "button";
  restart.dataset.action = "restart";
  restart.dataset.server = row.name;
  head.append(restart);

  const autoLabel = el("label", "toggle");
  const auto = el("input") as HTMLInputElement;
  auto.type = "checkbox";
  auto.checked = row.autorestart;
  auto.dataset.action = "autorestart";
  auto.dataset.server = row.name;
  autoLabel.append(auto, el("span", "muted small", "auto-restart"));
  head.append(autoLabel);

  item.append(head);

  if (row.error) {
    item.append(el("p", "error server-error", row.error));
  }

  if (row.tools.length > 0) {
    const details = el("details", "tools");
    details.open = row.status === "ready" && row.tools.length <= 3;
    const summary = el(
      "summary",
      undefined,
      `${row.tools.length} tool${row.tools.length === 1 ? "" : "s"}`,
    );
    details.append(summary);

    const list = el("ul", "tool-list");
    for (const tool of row.tools) {
      const li = el("li", "tool");
      li.title = `${tool.name}${requiredList(tool)}`;
      li.append(el("span", "tool-name", tool.name));
      if (tool.read_only) li.append(el("span", "secret", "read-only"));
      if (tool.description) li.append(el("span", "tool-desc", tool.description));
      li.append(policySelect(row, tool));
      list.append(li);
    }
    details.append(list);
    item.append(details);
  }

  return item;
}

function pendingNode(pending: Pending): HTMLLIElement {
  const item = el("li", "approval");
  const head = el("div", "server-head");
  head.append(el("span", "server-name", `${pending.server} / ${pending.tool}`));
  item.append(head);

  const args = pending.arguments ? JSON.stringify(pending.arguments) : "{}";
  const preview = args.length > 160 ? `${args.slice(0, 157)}…` : args;
  item.append(el("code", "url", preview));

  const row = el("div", "row");
  const approve = el("button", "btn btn-primary btn-small", "Approve");
  approve.type = "button";
  approve.dataset.action = "approve";
  approve.dataset.id = String(pending.id);
  const deny = el("button", "btn btn-small", "Deny");
  deny.type = "button";
  deny.dataset.action = "deny";
  deny.dataset.id = String(pending.id);
  row.append(approve, deny);
  item.append(row);
  return item;
}

function renderApprovals(list: Pending[]): void {
  refs.approvalsCard.hidden = list.length === 0;
  refs.approvalsCount.textContent = String(list.length);
  refs.approvalsList.replaceChildren(...list.map(pendingNode));
}

function render(rows: ServerRow[]): void {
  refs.servers.replaceChildren(...rows.map(serverNode));
  refs.serverCount.textContent = String(rows.length);
  refs.serversEmpty.hidden = rows.length > 0;
  refs.serversEmpty.textContent = "No servers registered yet. Paste JSON below.";
}

function renderDashboard(data: Dashboard): void {
  refs.renderer.textContent = data.renderer;
  refs.approvals.textContent = data.approvals_default;
  refs.registryPath.textContent = data.registry_path;
  refs.hubUrl.textContent = data.hub.url;
  refs.hubSessions.textContent = `${data.hub.sessions} session${data.hub.sessions === 1 ? "" : "s"}`;
  refs.hubNote.textContent = data.hub.note;
  renderTunnel(data.tunnel);
  renderApprovals(data.approvals);
  render(data.servers);
}

function showAddError(message: string): void {
  refs.addError.textContent = message;
}

async function load(): Promise<void> {
  if (!bridge) {
    refs.serversEmpty.textContent = "Tauri bridge not found — open the app, not the file.";
    return;
  }
  try {
    renderDashboard(await bridge.core.invoke<Dashboard>("dashboard"));
  } catch (error) {
    refs.serversEmpty.textContent = `dashboard failed: ${String(error)}`;
  }
}

async function refresh(): Promise<void> {
  if (!bridge) return;
  refs.refresh.disabled = true;
  refs.refresh.textContent = "Probing…";
  try {
    render(await bridge.core.invoke<ServerRow[]>("refresh"));
  } catch (error) {
    refs.serversEmpty.hidden = false;
    refs.serversEmpty.textContent = `refresh failed: ${String(error)}`;
  } finally {
    refs.refresh.disabled = false;
    refs.refresh.textContent = "Refresh";
  }
}

async function submit(): Promise<void> {
  if (!bridge) return;
  const json = refs.addJson.value.trim();
  if (!json) {
    showAddError("paste some JSON first");
    return;
  }
  showAddError("");
  refs.addSubmit.disabled = true;
  refs.addSubmit.textContent = "Spawning…";
  try {
    render(await bridge.core.invoke<ServerRow[]>("add_servers", { json }));
    refs.addJson.value = "";
  } catch (error) {
    showAddError(String(error));
  } finally {
    refs.addSubmit.disabled = false;
    refs.addSubmit.textContent = "Add and spawn";
  }
}

function renderTunnel(info: TunnelInfo): void {
  const pill = `pill pill-${info.status === "off" ? "idle" : info.status}`;
  refs.tunnelStatus.className = pill;
  refs.tunnelStatus.textContent = info.status;
  refs.tunnelUrl.textContent = info.public_url ?? "no public URL yet";
  refs.tunnelCopy.disabled = !info.public_url;
  const busy = info.status === "starting";
  refs.tunnelStart.disabled = busy || info.status === "ready";
  refs.tunnelStart.textContent = busy ? "Starting…" : "Start tunnel";
  refs.tunnelStop.disabled = !busy && info.status !== "ready";
  refs.tunnelError.textContent = info.error ?? "";
  refs.tunnelNote.textContent =
    info.status === "ready"
      ? "Public route live. Anyone with this URL can call the hub: demo data only, no auth."
      : "No public route. The tunnel exposes the hub as-is: demo data only, no auth.";
}

async function copy(text: string, button: HTMLButtonElement): Promise<void> {
  const restore = button.textContent;
  let ok = false;
  try {
    await navigator.clipboard.writeText(text);
    ok = true;
  } catch {
    const helper = document.createElement("textarea");
    helper.value = text;
    helper.style.position = "fixed";
    helper.style.opacity = "0";
    document.body.append(helper);
    helper.select();
    ok = document.execCommand("copy");
    helper.remove();
  }
  button.textContent = ok ? "Copied" : "Copy failed";
  window.setTimeout(() => {
    button.textContent = restore;
  }, 1400);
}

async function tunnelStart(): Promise<void> {
  if (!bridge) return;
  refs.tunnelError.textContent = "";
  refs.tunnelStart.disabled = true;
  refs.tunnelStart.textContent = "Starting…";
  try {
    renderTunnel(await bridge.core.invoke<TunnelInfo>("tunnel_start"));
  } catch (error) {
    refs.tunnelError.textContent = String(error);
    refs.tunnelStart.disabled = false;
    refs.tunnelStart.textContent = "Start tunnel";
  }
}

async function tunnelStop(): Promise<void> {
  if (!bridge) return;
  renderTunnel(await bridge.core.invoke<TunnelInfo>("tunnel_stop"));
}

async function setPolicy(select: HTMLSelectElement): Promise<void> {
  if (!bridge) return;
  const server = select.dataset.server ?? "";
  const tool = select.dataset.tool ?? "";
  const decision = select.value as Decision;
  select.disabled = true;
  try {
    render(await bridge.core.invoke<ServerRow[]>("set_policy", { server, tool, decision }));
  } catch (error) {
    showAddError(`policy failed: ${String(error)}`);
  } finally {
    select.disabled = false;
  }
}

async function setAutorestart(input: HTMLInputElement): Promise<void> {
  if (!bridge) return;
  const server = input.dataset.server ?? "";
  input.disabled = true;
  try {
    render(await bridge.core.invoke<ServerRow[]>("set_autorestart", { server, enabled: input.checked }));
  } catch (error) {
    input.checked = !input.checked;
    showAddError(`auto-restart failed: ${String(error)}`);
  } finally {
    input.disabled = false;
  }
}

async function restartServer(button: HTMLButtonElement): Promise<void> {
  if (!bridge) return;
  const server = button.dataset.server ?? "";
  button.disabled = true;
  button.textContent = "Restarting…";
  try {
    render(await bridge.core.invoke<ServerRow[]>("restart_server", { server }));
  } catch (error) {
    showAddError(`restart failed: ${String(error)}`);
  } finally {
    button.disabled = false;
    button.textContent = "Restart";
  }
}

async function resolveApproval(button: HTMLButtonElement, approve: boolean): Promise<void> {
  if (!bridge) return;
  const id = Number(button.dataset.id ?? 0);
  button.disabled = true;
  try {
    await bridge.core.invoke<boolean>("resolve_approval", { id, approve });
    renderApprovals(await bridge.core.invoke<Pending[]>("approvals"));
  } catch (error) {
    showAddError(`approval failed: ${String(error)}`);
  }
}

refs.refresh.addEventListener("click", () => void refresh());
refs.tunnelStart.addEventListener("click", () => void tunnelStart());
refs.tunnelStop.addEventListener("click", () => void tunnelStop());
refs.hubCopy.addEventListener("click", () => void copy(refs.hubUrl.textContent ?? "", refs.hubCopy));
refs.tunnelCopy.addEventListener("click", () =>
  void copy(refs.tunnelUrl.textContent ?? "", refs.tunnelCopy),
);
refs.addSubmit.addEventListener("click", () => void submit());

refs.servers.addEventListener("click", (event) => {
  const target = event.target as HTMLElement | null;
  const button = target?.closest<HTMLButtonElement>("button[data-action='restart']");
  if (button) void restartServer(button);
});

refs.servers.addEventListener("change", (event) => {
  const target = event.target as HTMLElement | null;
  if (target instanceof HTMLSelectElement && target.classList.contains("policy")) {
    void setPolicy(target);
  } else if (target instanceof HTMLInputElement && target.dataset.action === "autorestart") {
    void setAutorestart(target);
  }
});

refs.approvalsList.addEventListener("click", (event) => {
  const target = event.target as HTMLElement | null;
  if (!(target instanceof HTMLButtonElement)) return;
  if (target.dataset.action === "approve") void resolveApproval(target, true);
  if (target.dataset.action === "deny") void resolveApproval(target, false);
});

refs.addFile.addEventListener("change", () => {
  const file = refs.addFile.files?.[0];
  if (!file) return;
  file
    .text()
    .then((text) => {
      refs.addJson.value = text;
      showAddError("");
    })
    .catch((error: unknown) => showAddError(`cannot read file: ${String(error)}`));
});

if (bridge) {
  void bridge.event.listen<ServerRow[]>("servers-updated", (event) => render(event.payload));
  void bridge.event.listen<TunnelInfo>("tunnel-updated", (event) =>
    renderTunnel(event.payload),
  );
  void bridge.event.listen<Pending>("approval-requested", () => void refreshApprovals());
  // Prompts also expire (60s) without any event reaching us.
  window.setInterval(() => void refreshApprovals(), 5000);
}

async function refreshApprovals(): Promise<void> {
  if (!bridge) return;
  try {
    renderApprovals(await bridge.core.invoke<Pending[]>("approvals"));
  } catch {
    // the banner is best-effort; the next tick retries
  }
}

void load();
