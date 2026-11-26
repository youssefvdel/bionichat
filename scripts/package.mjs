// Pack a runnable folder into one archive: the app, its tool servers, the
// tunnel helper and (for a CEF build) the Chromium runtime, all beside each
// other the way the app expects them.
//
//   bun run package            -> dist/bionichat-<platform>-<version>.zip
import { execFileSync } from "node:child_process";
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { basename, join, resolve } from "node:path";

const exe = process.platform === "win32" ? ".exe" : "";
const files = ["bionichat", "core", "bionichat-tunnel"].map((name) => name + exe);
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
// The runtime ships in a directory of its own (cef_windows_x86_64,
// cef_linux_x86_64, Chromium Embedded Framework.framework). cargo also drops
// a libcef.dll into cef-dll-sys/<hash>/out/ next to 150 MB of rlibs, so match
// the directory name, never just the library.
function isRuntimeDir(dir) {
  const base = basename(dir);
  if (!base.startsWith("cef_") && !base.endsWith(".framework")) return false;
  return ["libcef.dll", "libcef.so", "Chromium Embedded Framework"].some((name) => existsSync(join(dir, name)));
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

const archiveDir = resolve("dist");
mkdirSync(archiveDir, { recursive: true });
const archive = join(archiveDir, `bionichat-${platform}-${version}.zip`);
if (process.platform === "win32") {
  // Compress-Archive is slow on a 276 MB libcef.dll, and the `tar` on PATH in
  // Git Bash is GNU tar, which reads `D:\...` as a remote host. Use the
  // bsdtar Windows itself ships, by absolute path.
  const bsdtar = "C:\\Windows\\System32\\tar.exe";
  const tar = existsSync(bsdtar) ? bsdtar : "tar";
  execFileSync(tar, ["-a", "-c", "-f", archive, "-C", stage, "."], { stdio: "inherit" });
} else {
  execFileSync("zip", ["-rq", archive, "."], { cwd: stage, stdio: "inherit" });
}
console.log(`\n${archive}`);
