// Dashboard for web-agent pass 1: registry-backed MCP server list.
// No framework, no npm deps — bun compiles this to dist/main.js.

type Status = "pending" | "ready" | "error";

interface ToolInfo {
  name: string;
  description: string | null;
  read_only: boolean;
}

interface ServerRow {
  name: string;
  transport: string;
  secret_keys: string[];
  status: Status;
  error: string | null;
  tools: ToolInfo[];
}

interface Dashboard {
  servers: ServerRow[];
  tunnel: { status: string; public_url: string | null; note: string };
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
  tunnelStatus: qs<HTMLSpanElement>("#tunnel-status"),
  tunnelNote: qs<HTMLParagraphElement>("#tunnel-note"),
  tunnelUrl: qs<HTMLElement>("#tunnel-url"),
  tunnelStart: qs<HTMLButtonElement>("#tunnel-start"),
  serverCount: qs<HTMLSpanElement>("#server-count"),
  servers: qs<HTMLUListElement>("#servers"),
  serversEmpty: qs<HTMLParagraphElement>("#servers-empty"),
  approvals: qs<HTMLElement>("#approvals"),
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
      li.append(el("span", "tool-name", tool.name));
      if (tool.read_only) li.append(el("span", "secret", "read-only"));
      if (tool.description) li.append(el("span", "tool-desc", tool.description));
      list.append(li);
    }
    details.append(list);
    item.append(details);
  }

  return item;
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
  refs.tunnelStatus.textContent = data.tunnel.status;
  refs.tunnelNote.textContent = data.tunnel.note;
  refs.tunnelUrl.textContent = data.tunnel.public_url ?? "no public URL yet";
  refs.tunnelStart.disabled = true;
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

refs.refresh.addEventListener("click", () => void refresh());
refs.addSubmit.addEventListener("click", () => void submit());

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
}

void load();
