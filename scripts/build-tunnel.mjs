// Build the tunnel helper and put it beside the app binary.
//
// The helper is bionichat's own OpenTunnel client (tools/tunnel). It has to sit
// next to bionichat, because that is where the app looks for it.
//
//   bun run tunnel:build                 host target
//   bun run tunnel:build --target x86_64-pc-windows-gnu
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, readdirSync, chmodSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";

const args = process.argv.slice(2);
const targetFlag = args.indexOf("--target");
const target = targetFlag === -1 ? null : args[targetFlag + 1];
const isWindows = target ? target.includes("windows") : process.platform === "win32";
const name = isWindows ? "bionichat-tunnel.exe" : "bionichat-tunnel";

const cargo = ["build", "--release", "-p", "bionichat-tunnel"];
if (target) cargo.push("--target", target);
console.log(`tunnel: cargo ${cargo.join(" ")}`);
execFileSync("cargo", cargo, { stdio: "inherit" });

const built = target
  ? join("target", target, "release", name)
  : join("target", "release", name);
if (!existsSync(built)) throw new Error(`helper was not built at ${built}`);

// Copy beside every app binary in the tree (plain target, and custom ones).
const appName = name.includes(".exe") ? "bionichat.exe" : "bionichat";
const placed = [];
for (const root of ["src-tauri/target", "target"]) {
  if (!existsSync(root)) continue;
  for (const entry of readdirSync(root)) {
    const direct = join(root, entry);
    const dirs = [direct];
    if (!entry.startsWith("debug") && !entry.startsWith("release")) {
      for (const sub of ["debug", "release"]) dirs.push(join(direct, sub));
    }
    for (const dir of dirs) {
      if (!existsSync(join(dir, appName))) continue;
      const destination = join(dir, name);
      // When the app lives in the same folder as the build output, the helper
      // is already in place: copying it onto itself would delete it.
      if (resolve(destination) === resolve(built)) {
        placed.push(destination);
        continue;
      }
      // A running helper holds its file open; skip it rather than fail the build.
      try {
        rmSync(destination, { force: true });
        copyFileSync(built, destination);
      } catch (error) {
        console.log(`tunnel: ${destination} is in use (${error.code}); stop the app and rerun`);
        continue;
      }
      if (!name.endsWith(".exe")) chmodSync(destination, 0o755);
      placed.push(destination);
    }
  }
}

if (placed.length === 0) console.log(`tunnel: built ${built}, but no app binary to sit beside yet`);
for (const path of placed) console.log(`tunnel: placed ${path}`);
