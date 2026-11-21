import { useState } from "react";
import type { TunnelInfo } from "../types";
import { invoke } from "../tauri";
import { Card, ActionButton } from "./ui";
import { Pill } from "./chrome";

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const helper = document.createElement("textarea");
    helper.value = text;
    helper.style.position = "fixed";
    helper.style.opacity = "0";
    document.body.append(helper);
    helper.select();
    const ok = document.execCommand("copy");
    helper.remove();
    return ok;
  }
}

function CopyButton({
  text,
  disabled,
  primary,
}: {
  text: string;
  disabled?: boolean;
  primary?: boolean;
}) {
  const [label, setLabel] = useState("Copy URL");
  return (
    <ActionButton
      primary={primary}
      disabled={disabled}
      onClick={() => {
        if (!text) return;
        void copyText(text).then((ok) => {
          setLabel(ok ? "Copied" : "Copy failed");
          window.setTimeout(() => setLabel("Copy URL"), 1400);
        });
      }}
    >
      {label}
    </ActionButton>
  );
}

export function TunnelCard({
  info,
  onChange,
}: {
  info: TunnelInfo;
  onChange: (info: TunnelInfo) => void;
}) {
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState("");
  const [armed, setArmed] = useState(false);

  const busy = info.status === "starting" || starting;
  const live = info.status === "ready";

  async function start() {
    setError("");
    setStarting(true);
    try {
      onChange(await invoke<TunnelInfo>("tunnel_start"));
    } catch (e) {
      setError(String(e));
    } finally {
      setStarting(false);
    }
  }

  async function stop() {
    onChange(await invoke<TunnelInfo>("tunnel_stop"));
  }

  async function revoke() {
    if (!armed) {
      setArmed(true);
      window.setTimeout(() => setArmed(false), 6000);
      return;
    }
    setArmed(false);
    onChange(await invoke<TunnelInfo>("tunnel_revoke"));
  }

  return (
    <Card
      labelledBy="tunnel-title"
      title="Tunnel"
      right={<Pill status={busy ? "starting" : info.status} />}
    >
      <p className="m-0 text-[12.5px] text-dim">
        {live
          ? "Public route live. Anyone with this URL can call the hub: demo data only, no auth."
          : "No public route. The tunnel exposes the hub as-is: demo data only, no auth."}
      </p>
      <code className="mt-3 block w-full font-mono text-[18px] leading-snug break-all text-ink">
        {info.public_url ?? "no public URL yet"}
      </code>
      <div className="mt-3 flex flex-wrap items-center gap-2.5">
        {/* The one primary action follows the state: copy while live,
            start while down. A disabled primary outranks nothing. */}
        <CopyButton
          text={info.public_url ?? ""}
          disabled={!info.public_url}
          primary={live}
        />
        {!live && (
          <ActionButton primary disabled={busy} onClick={() => void start()}>
            {busy ? "Starting…" : "Start tunnel"}
          </ActionButton>
        )}
        {live && (
          <>
            <ActionButton onClick={() => void stop()}>Stop</ActionButton>
            <ActionButton
              onClick={() => void revoke()}
              title="Burn this URL for good; next start mints a fresh one"
            >
              {armed ? "Sure? Click again to burn" : "Revoke link"}
            </ActionButton>
          </>
        )}
      </div>
      {(error || info.error) && (
        <p role="alert" className="mt-2 font-mono text-xs break-words text-bad">
          {error || info.error}
        </p>
      )}
    </Card>
  );
}
