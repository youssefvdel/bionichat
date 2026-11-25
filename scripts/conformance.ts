// Black-box conformance run against a live bionichat link, from outside the
// app: raw MCP over HTTPS, the exact requests a chat client sends.
//
//   bun scripts/conformance.ts [url]
//
// Default target is the Windows machine's public tunnel.

import { resolve } from "node:path";

const url = process.argv[2] ?? "https://lzite2fohvo7.opentunnel.xyz/mcp";
// One candidate per platform: the target machine is whichever link we hit.
const SHOTS = [
  process.env.BIONICHAT_IMAGE,
  // Shipped in the repo, so the image checks always have a subject.
  resolve("docs/screenshot.png"),
  "C:\\Users\\Administrator\\Pictures\\Screenshots\\Screenshot 2026-11-22 072557.png",
  "/tmp/opencode/v1.png",
].filter((value): value is string => Boolean(value));
const DIR = "C:\\Users\\Administrator\\bionichat-conformance";

type Rpc = { jsonrpc: "2.0"; id: number; method: string; params?: unknown };
type Row = { step: string; ok: boolean; detail: string; ms: number };

let nextId = 1;
const rows: Row[] = [];

async function rpc(method: string, params?: unknown) {
  const body: Rpc = { jsonrpc: "2.0", id: nextId++, method, params };
  const started = performance.now();
  const response = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  const text = await response.text();
  const ms = Math.round(performance.now() - started);
  const parsed = text ? JSON.parse(text) : null;
  return { status: response.status, body: parsed, ms, raw: text };
}

function record(step: string, ok: boolean, detail: string, ms: number) {
  rows.push({ step, ok, detail, ms });
  console.log(`${ok ? "PASS" : "FAIL"}  ${step}  ${detail}  ${ms} ms`);
}

async function call(name: string, args: Record<string, unknown>) {
  return rpc("tools/call", { name, arguments: args });
}

type Shell = { exit_code: number; stdout: string; stderr: string };

function shellOf(body: any): Shell | null {
  const text = textOf(body);
  const start = text.indexOf("{");
  const end = text.lastIndexOf("}");
  if (start < 0 || end < 0) return null;
  try {
    return JSON.parse(text.slice(start, end + 1)) as Shell;
  } catch {
    return null;
  }
}

function firstLine(text: string): string {
  const line = text.split("\n").find((value) => value.trim().length > 0) ?? "";
  return line.slice(0, 70);
}

function textOf(body: any): string {
  const content = body?.result?.content ?? [];
  return content
    .map((block: any) => (block.type === "text" ? block.text : `<${block.type}>`))
    .join("\n");
}

async function main() {
  // 1. Handshake
  const init = await rpc("initialize", {
    protocolVersion: "2025-11-25",
    capabilities: {},
    clientInfo: { name: "conformance", version: "1" },
  });
  const info = init.body?.result?.serverInfo ?? {};
  record(
    "initialize",
    init.status === 200 && typeof init.body?.result?.instructions === "string",
    `${info.name} / ${info.title}`,
    init.ms,
  );
  record(
    "serverInfo describes the PC",
    String(info.description ?? "").includes("user's own computer"),
    String(info.description ?? "").slice(0, 48) + "...",
    0,
  );

  // 2. Catalog
  const list = await rpc("tools/list");
  const tools: string[] = (list.body?.result?.tools ?? []).map((t: any) => t.name);
  const wanted = ["core_read", "core_write", "core_edit", "core_bash", "core_skill"];
  record(
    "tools/list",
    wanted.every((name) => tools.includes(name)),
    `${tools.length} tools: ${tools.join(", ")}`,
    list.ms,
  );
  record(
    "descriptions carry no approval wording",
    (list.body?.result?.tools ?? []).every(
      (t: any) => !/approval required|needs approval/i.test(t.description ?? ""),
    ),
    "no gate text",
    0,
  );

  // Pick paths for the platform behind the link before touching files.
  const probe = await call("core_bash", { command: "echo %OS%" });
  const isWindows = (shellOf(probe.body)?.stdout ?? "").includes("Windows_NT");
  const path = isWindows ? `${DIR}\\probe.txt` : "/tmp/opencode/bionichat-conformance/probe.txt";
  const images = SHOTS;
  record("platform detected", true, isWindows ? "Windows" : "POSIX", probe.ms);

  // 3. Write
  const write = await call("core_write", {
    path,
    content: "line one\nline two\nline three\n",
  });
  record(
    "core_write",
    write.body?.result?.isError === false && /wrote \d+ bytes/.test(textOf(write.body)),
    textOf(write.body).split("\n")[0],
    write.ms,
  );

  // 4. Read back, numbered
  const read = await call("core_read", { path });
  const readText = textOf(read.body);
  record(
    "core_read",
    readText.includes("1: line one") && /\[server time .+\]/.test(readText),
    readText.split("\n").find((l) => l.includes("line one")) ?? readText.slice(0, 40),
    read.ms,
  );

  // 5. Edit exact text
  const edit = await call("core_edit", {
    path,
    oldString: "line two",
    newString: "line two edited",
  });
  const after = await call("core_read", { path });
  record(
    "core_edit",
    textOf(after.body).includes("line two edited"),
    textOf(edit.body).split("\n")[0],
    edit.ms,
  );

  // 6. Edit with a bad anchor must fail without writing
  const badEdit = await call("core_edit", {
    path,
    oldString: "no such text anywhere",
    newString: "x",
  });
  record(
    "core_edit rejects a missing anchor",
    badEdit.body?.result?.isError === true,
    textOf(badEdit.body).split("\n")[0].slice(0, 70),
    badEdit.ms,
  );

  // 7. Shell
  const bash = await call("core_bash", {
    command: 'echo conformance-ok && echo "quoted  path" && cd',
  });
  const shell = shellOf(bash.body);
  record(
    "core_bash",
    shell?.exit_code === 0 && shell.stdout.includes("conformance-ok"),
    `exit ${shell?.exit_code} · ${(shell?.stdout ?? "").trim().split("\n")[0]}`,
    bash.ms,
  );
  record(
    "core_bash keeps quotes intact",
    shell?.stdout.includes("quoted  path") === true,
    `exit ${shell?.exit_code}`,
    bash.ms,
  );

  // 8. Skills
  const skills = await call("core_skill", { action: "list" });
  record(
    "core_skill list",
    skills.body?.result?.isError === false,
    textOf(skills.body).split("\n")[0].slice(0, 60),
    skills.ms,
  );

  // 9. Image: block, then fetch its published URL byte for byte
  let image = await call("core_read", { path: images[0] });
  for (const candidate of images.slice(1)) {
    const blocks = image.body?.result?.content ?? [];
    if (blocks[0]?.type === "image") break;
    image = await call("core_read", { path: candidate });
  }
  const blocks = image.body?.result?.content ?? [];
  const first = blocks[0] ?? {};
  const imageText = blocks
    .filter((b: any) => b.type === "text")
    .map((b: any) => b.text)
    .join("\n");
  record(
    "core_read image block first",
    first.type === "image" && first.mimeType === "image/png",
    `${first.mimeType ?? "none"} ${String(first.data ?? "").length} base64 chars`,
    image.ms,
  );
  const link = imageText.split("\n").find((line) => line.startsWith("Image URL: "));
  const imageUrl = link?.replace("Image URL: ", "").trim();
  if (imageUrl) {
    const started = performance.now();
    const fetched = await fetch(imageUrl);
    const bytes = new Uint8Array(await fetched.arrayBuffer());
    const isPng =
      bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47;
    record(
      "published image URL",
      fetched.status === 200 && isPng,
      `${fetched.status} ${fetched.headers.get("content-type")} ${bytes.length} bytes`,
      Math.round(performance.now() - started),
    );
  } else {
    record("published image URL", false, "no Image URL line", 0);
  }

  // 9b. PDF: extracted text, not bytes
  const pdfs = isWindows
    ? ["C:\\Users\\Administrator\\Downloads\\26CSCI08I_SPM_CW_Term_Vlad.pdf"]
    : ["/home/youssefvdel/Documents/exams-unlocked/computer arch final exam.pdf"];
  for (const pdf of pdfs) {
    const doc = await call("core_read", { path: pdf });
    const text = textOf(doc.body);
    if (/is not a file|No such file|cannot find/i.test(text)) continue;
    record(
      "core_read extracts PDF text",
      text.includes("is a PDF") && text.length > 500,
      text.split("\n")[0].slice(0, 80),
      doc.ms,
    );
    break;
  }

  // 9c. Office files: text out of a docx and an xlsx, no client support needed
  const officeCases = isWindows
    ? [
        { kind: "docx", path: "C:\\Users\\Administrator\\Downloads\\26CSCI05I_Report_Template_CLEAN.docx" },
        { kind: "xlsx", path: "C:\\Users\\Administrator\\Downloads\\Audit logs\\AUDIT moodle sync app.xlsx" },
      ]
    : [
        { kind: "docx", path: "/tmp/opencode/note.docx" },
        { kind: "xlsx", path: "/tmp/opencode/marks.xlsx" },
      ];
  for (const office of officeCases) {
    const doc = await call("core_read", { path: office.path });
    const text = textOf(doc.body);
    if (/No such file|not a file|could not be read: No such/i.test(text)) continue;
    record(
      `core_read reads .${office.kind} as text`,
      text.includes(`is a ${office.kind} document`),
      text.split("\n")[0].slice(0, 70),
      doc.ms,
    );
  }

  // 10. Errors behave like MCP
  const unknown = await call("zzz_nope", {});
  record(
    "unknown tool -> -32602",
    unknown.body?.error?.code === -32602,
    String(unknown.body?.error?.message ?? "").slice(0, 48),
    unknown.ms,
  );

  // 11. Clean up the probe file
  const rm = await call("core_bash", {
    command: isWindows ? `del "${path}"` : `rm -f "${path}"`,
  });
  const clean = shellOf(rm.body);
  record(
    "cleanup",
    clean?.exit_code === 0,
    `exit ${clean?.exit_code} ${(clean?.stderr ?? "").trim()}`,
    rm.ms,
  );

  const failed = rows.filter((row) => !row.ok);
  const slowest = rows.reduce((a, b) => (b.ms > a.ms ? b : a));
  console.log(
    `\n${rows.length - failed.length}/${rows.length} passed · slowest ${slowest.step} ${slowest.ms} ms`,
  );
  if (failed.length) {
    console.log(`failed: ${failed.map((row) => row.step).join(", ")}`);
    process.exit(1);
  }
}

await main();
