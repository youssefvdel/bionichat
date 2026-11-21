// Builds dist/_debug.html: the real bundle, running against fixture data in a
// plain browser. Lets the UI be inspected in DevTools without the Tauri shell.
// Usage: bun run debug-page
import { readdirSync, writeFileSync } from "node:fs";

const files = readdirSync("dist/assets");
const js = files.find((f) => f.startsWith("index-") && f.endsWith(".js"));
const css = files.find((f) => f.startsWith("index-") && f.endsWith(".css"));
if (!js || !css) throw new Error("run vite build first");

const servers = [
  {
    name: "fs",
    transport: "stdio · /home/youssefvdel/Projects/web-agent/target/debug/fs",
    secret_keys: [],
    autorestart: false,
    status: "ready",
    error: null,
    tools: [
      { name: "read", description: "Reads a UTF-8 text file.", read_only: true, input_schema: {}, policy: "allow" },
      { name: "write", description: "Writes a file.", read_only: false, input_schema: {}, policy: "ask" },
      { name: "edit", description: "Replaces exact text.", read_only: false, input_schema: {}, policy: "ask" },
    ],
  },
  {
    name: "bash",
    transport: "stdio · /home/youssefvdel/Projects/web-agent/target/debug/bash",
    secret_keys: [],
    autorestart: false,
    status: "ready",
    error: null,
    tools: [
      { name: "bash", description: "Runs a shell command.", read_only: false, input_schema: {}, policy: "ask" },
    ],
  },
  {
    name: "skill",
    transport: "stdio · /home/youssefvdel/Projects/web-agent/target/debug/skill",
    secret_keys: [],
    autorestart: false,
    status: "ready",
    error: null,
    tools: [
      { name: "skill", description: "Lists or loads a skill.", read_only: true, input_schema: {}, policy: "allow" },
    ],
  },
];

const fixtures = {
  dashboard: {
    servers,
    hub: { url: "http://127.0.0.1:8787/mcp", port: 8787, sessions: 3, note: "JSON-RPC over POST, no SSE" },
    tunnel: {
      status: "ready",
      public_url: "https://c836bdb2bc68b0d5.kfgepc72im6k.opentunnel.xyz/mcp",
      error: null,
    },
    approvals: [],
    approvals_default: "read-only allow · everything else ask",
    renderer: "cef",
    registry_path: "/home/youssefvdel/.local/share/com.bionichat.app/registry.json",
  },
  approvals: [],
  activity: [],
};

const html = `<!doctype html>
<html><head><meta charset="utf-8"><link rel="stylesheet" href="/assets/${css}"></head>
<body><div id="root"></div>
<script>
const fixtures = ${JSON.stringify(fixtures)};
const listeners = {};
window.__TAURI__ = {
  core: { invoke: async (name) => fixtures[name] },
  event: { listen: async (name, handler) => { (listeners[name] ||= []).push(handler); return () => {}; } }
};
window.__fire = (name, payload) => (listeners[name] || []).forEach((h) => h({ payload }));
</script>
<script type="module" src="/assets/${js}"></script>
</body></html>`;

writeFileSync("dist/_debug.html", html);
console.log("wrote dist/_debug.html");
