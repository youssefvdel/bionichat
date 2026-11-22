// Pack a runnable folder into one archive: the app, its tool servers and the
// tunnel helper, all beside each other the way the app expects them.
//
//   bun run package            -> dist/bionichat-<platform>-<version>.zip
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";

const exe = process.platform === "win32" ? ".exe" : "";
const files = ["bionichat", "fs", "bash", "skill", "cloudflared"].map((name) => name + exe);
const version = JSON.parse(readFileSync("package.json", "utf8")).version ?? "0.1.0";
const platform = `${process.platform}-${process.arch}`;

const candidates = ["target/release", "src-tauri/target/release", "target/debug"];
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

const archive = join("dist", `bionichat-${platform}-${version}.zip`);
if (process.platform === "win32") {
  execFileSync("powershell", ["-NoProfile", "-Command", `Compress-Archive -Path '${stage}\\*' -DestinationPath '${archive}' -Force`], { stdio: "inherit" });
} else {
  execFileSync("zip", ["-rq", archive, "."], { cwd: stage, stdio: "inherit" });
}
console.log(`\n${archive}`);
