// Pack a runnable folder into one archive: the app, its tool servers, the
// tunnel helper and (for a CEF build) the Chromium runtime, all beside each
// other the way the app expects them.
//
//   bun run package            -> dist/bionichat-<platform>-<version>.zip
import { execFileSync } from "node:child_process";
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const exe = process.platform === "win32" ? ".exe" : "";
const files = ["bionichat", "fs", "bash", "skill", "bionichat-tunnel"].map((name) => name + exe);
const version = JSON.parse(readFileSync("package.json", "utf8")).version ?? "0.1.0";
const platform = `${process.platform}-${process.arch}`;

// Both layouts exist in the wild: the workspace target/ and a src-tauri/target/
// left behind by a --manifest-path build. Take the newest app binary and keep
// every other file next to it, so a stale copy can never win.
const roots = ["target", "src-tauri/target"].filter((root) => existsSync(root));
const builds = roots
  .map((root) => {
    for (const profile of ["release", "debug"]) {
      const dir = join(root, profile);
      if (existsSync(join(dir, files[0]))) return { root, profile, dir, at: statSync(join(dir, files[0])).mtimeMs };
    }
    return null;
  })
  .filter((build) => build !== null)
  .sort((a, b) => b.at - a.at);

const build = builds[0];
if (!build) throw new Error("no build found under target/ or src-tauri/target/; run cargo build first");
const { root, profile, dir: source } = build;
console.log(`packing ${source} (${profile})`);

// Never stage inside dist/: tauri embeds ../dist as the frontend, so a staged
// Chromium runtime there would be baked into the next app build (and its
// CMakeLists.txt would fail the asset read).
const stage = join("target", "package", "bionichat-" + platform);
rmSync(stage, { recursive: true, force: true });
mkdirSync(stage, { recursive: true });

for (const file of files) {
  const from = join(source, file);
  if (!existsSync(from)) throw new Error(`${file} missing from ${source}`);
  copyFileSync(from, join(stage, file));
  console.log(`packed ${file}`);
}

// A CEF build needs its Chromium runtime beside the exe: libcef, the .pak
// resources, locales and (on Linux) the setuid sandbox helper. Only the
// profile that produced the binaries: a debug CEF is 1.4 GB.
//
// The build layout differs by crate version: `cef-dll-sys-<hash>/out/cef_*`
// and `cef-dll-sys/<hash>/out/cef_*` both exist in the wild, so find the
// library itself and take its directory instead of guessing the path.
// A runtime directory is the one that also carries Chromium's assets. A bare
// libcef.dll also appears higher up in build/, next to 60 GB of cargo output:
// copying that would ship half the build tree.
function isRuntimeDir(dir) {
  const hasLibrary = ["libcef.dll", "libcef.so", "Chromium Embedded Framework.framework"].some((name) =>
    existsSync(join(dir, name)),
  );
  if (!hasLibrary) return false;
  if (existsSync(join(dir, "locales"))) return true;
  return readdirSync(dir).some((name) => name.endsWith(".pak"));
}

function findCefDir(dir, depth) {
  if (depth > 5 || !existsSync(dir)) return null;
  if (isRuntimeDir(dir)) return dir;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const found = findCefDir(join(dir, entry.name), depth + 1);
    if (found) return found;
  }
  return null;
}

const cefDir = findCefDir(join(root, profile, "build"), 0);
const cefDirs = cefDir ? [cefDir] : [];

for (const dir of cefDirs) {
  cpSync(dir, stage, { recursive: true, force: true });
  console.log(`packed the CEF runtime from ${dir}`);
}

const archive = resolve("dist", `bionichat-${platform}-${version}.zip`);
if (process.platform === "win32") {
  execFileSync("powershell", ["-NoProfile", "-Command", `Compress-Archive -Path '${stage}\\*' -DestinationPath '${archive}' -Force`], { stdio: "inherit" });
} else {
  execFileSync("zip", ["-rq", archive, "."], { cwd: stage, stdio: "inherit" });
}
console.log(`\n${archive}`);
