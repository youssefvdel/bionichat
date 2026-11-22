// Fetch the tunnel helper for this platform and put it where the app looks for
// it: next to the built binary, so a copy of the build folder needs nothing
// installed. Run before cargo build (bun run tunnel:fetch).
//
// cloudflared ships as one self-contained binary per platform and gives an
// HTTPS URL without an account. OpenTunnel is preferred at runtime when the
// user already has it, because it keeps one stable URL per machine.
import { chmodSync, copyFileSync, existsSync, mkdirSync, statSync } from "node:fs";
import { join } from "node:path";

const RELEASE = "https://github.com/cloudflare/cloudflared/releases/latest/download";

function target() {
  const arch = process.arch === "arm64" ? "arm64" : "amd64";
  switch (process.platform) {
    case "linux":
      return { file: "cloudflared-linux-" + arch, name: "cloudflared" };
    case "darwin":
      return { file: "cloudflared-darwin-" + arch + ".tgz", name: "cloudflared", tgz: true };
    case "win32":
      return { file: "cloudflared-windows-" + arch + ".exe", name: "cloudflared.exe" };
    default:
      throw new Error(`unsupported platform: ${process.platform}`);
  }
}

const { file, name, tgz } = target();
const destinations = [];

// The app runs its own binary from whichever target directory cargo used
// (plain, or a custom one such as target/cef/debug). Put the helper beside
// every build that has an app binary in it.
const { readdirSync } = await import("node:fs");
const appName = process.platform === "win32" ? "bionichat.exe" : "bionichat";
const roots = [join("src-tauri", "target"), "target"];
for (const root of roots) {
  if (!existsSync(root)) continue;
  for (const entry of readdirSync(root)) {
    const direct = join(root, entry);
    const candidates = [direct];
    if (!entry.startsWith("debug") && !entry.startsWith("release")) {
      for (const sub of ["debug", "release"]) candidates.push(join(direct, sub));
    }
    for (const dir of candidates) {
      if (existsSync(join(dir, appName))) destinations.push(join(dir, name));
    }
  }
}

if (destinations.length === 0) destinations.push(join("src-tauri", "resources", name));

const missing = destinations.filter((path) => !existsSync(path));
for (const path of missing) {
  if (!missing.includes(path)) {
    console.log(`tunnel: ${path} already present (${Math.round(statSync(path).size / 1048576)} MB)`);
  }
}
if (missing.length === 0) process.exit(0);

console.log(`tunnel: downloading ${file}`);
const response = await fetch(`${RELEASE}/${file}`);
if (!response.ok) throw new Error(`download failed: ${response.status}`);
const bytes = new Uint8Array(await response.arrayBuffer());

for (const path of missing) {
  mkdirSync(join(path, ".."), { recursive: true });
  if (tgz) {
    throw new Error("darwin tgz extraction is not wired yet; install cloudflared with brew");
  }
  const { writeFileSync } = await import("node:fs");
  writeFileSync(path, bytes);
  if (process.platform !== "win32") chmodSync(path, 0o755);
  console.log(`tunnel: wrote ${path} (${Math.round(bytes.length / 1048576)} MB)`);
}
