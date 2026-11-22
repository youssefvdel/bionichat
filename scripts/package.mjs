// Pack a runnable folder into one archive: the app, its tool servers and the
// tunnel helper, all beside each other the way the app expects them.
//
//   bun run package            -> dist/bionichat-<platform>-<version>.zip
import { execFileSync } from "node:child_process";
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";

const exe = process.platform === "win32" ? ".exe" : "";
const files = ["bionichat", "fs", "bash", "skill", "bionichat-tunnel"].map((name) => name + exe);
const version = JSON.parse(readFileSync("package.json", "utf8")).version ?? "0.1.0";
const platform = `${process.platform}-${process.arch}`;

const candidates = ["src-tauri/target/release", "target/release", "src-tauri/target/debug", "target/debug"];
const source = candidates.find((dir) => existsSync(join(dir, files[0])));
if (!source) throw new Error(`no build found in ${candidates.join(", ")}; run cargo build first`);

const stage = join("dist", "bionichat-" + platform);
rmSync(stage, { recursive: true, force: true });
mkdirSync(stage, { recursive: true });

for (const file of files) {
  const from = join(source, file);
  if (!existsSync(from)) throw new Error(`${file} missing from ${source}`);
  copyFileSync(from, join(stage, file));
  console.log(`packed ${file}`);
}

// A CEF build needs its Chromium runtime beside the exe: libcef, the .pak
// resources, locales and (on Linux) the setuid sandbox helper.
// Only the profile that produced the binaries: a debug CEF is 1.4 GB.
const profile = source.includes("release") ? "release" : "debug";
const cefOut = [`target/${profile}/build`, `src-tauri/target/${profile}/build`]
  .filter((dir) => existsSync(dir))
  .flatMap((dir) =>
    readdirSync(dir)
      .filter((name) => name.startsWith("cef-dll-sys-"))
      .map((name) => join(dir, name, "out"))
      .flatMap((out) =>
        existsSync(out)
          ? readdirSync(out)
              .filter((name) => name.startsWith("cef_"))
              .map((name) => join(out, name))
          : [],
      ),
  )
  .filter((dir) => existsSync(join(dir, "libcef.dll")) || existsSync(join(dir, "libcef.so")) || existsSync(join(dir, "Chromium Embedded Framework.framework")));

for (const dir of cefOut) {
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
