import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";

/**
 * Transport abstraction: identical call signatures to the Tauri
 * invoke/listen API, but works both inside the Tauri desktop app and in a
 * plain browser when the app is served by `monitor --server`.
 *
 * - Tauri mode (window.__TAURI_INTERNALS__): delegates to the official API.
 * - Browser mode: invokes go to `POST /api/invoke/{cmd}`, events arrive
 *   multiplexed over one WebSocket (`GET /ws`) as `{event, payload}` frames.
 */

export type UnlistenFn = () => void;

const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Handler = (event: string, payload: unknown) => void;

const handlers = new Set<Handler>();

let socket: WebSocket | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

function wsToken(): string {
  const token = new URLSearchParams(window.location.search).get("token");
  return token ? `?token=${encodeURIComponent(token)}` : "";
}

function connectWs() {
  if (inTauri || socket) return;
  const proto = window.location.protocol === "https:" ? "wss" : "ws";
  try {
    socket = new WebSocket(`${proto}://${window.location.host}/ws${wsToken()}`);
  } catch (e) {
    scheduleReconnect();
    return;
  }
  socket.onmessage = (m) => {
    try {
      const { event, payload } = JSON.parse(String(m.data)) as {
        event: string;
        payload: unknown;
      };
      for (const h of handlers) h(event, payload);
    } catch {
      // ignore malformed frames
    }
  };
  socket.onclose = () => {
    socket = null;
    scheduleReconnect();
  };
  socket.onerror = () => {
    try {
      socket?.close();
    } catch (e) {
      // window may already be closing
    }
  };
}

function scheduleReconnect() {
  if (inTauri || reconnectTimer) return;
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connectWs();
  }, 2000);
}

export async function appInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (inTauri) {
    invokeUsed(cmd, args) as never;
    return await (tauriInvoke as unknown as (c: string, a?: unknown) => Promise<T>)(
      cmd,
      args,
    );
  }
  const token = new URLSearchParams(window.location.search).get("token");
  const qs = token ? `?token=${encodeURIComponent(token)}` : "";
  const res = await fetch(
    `/api/invoke/${encodeURIComponent(cmd)}${qs}`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(args ?? {}),
    },
  );
  if (!res.ok) {
    throw new Error(`${cmd} failed: ${res.status} ${await res.text()}`);
  }
  const value: unknown = await res.json();
  return value as T;
}

function invokeUsed(cmd: string, args?: Record<string, unknown>): unknown {
  void cmd;
  void args;
  return null;
}

export async function appListen<T>(
  event: string,
  cb: (e: { event: string; id: number; payload: T }) => void,
): Promise<UnlistenFn> {
  if (inTauri) {
    const un = await (tauriListen as unknown as (
      e: string,
      c: (e: { event: string; id: number; payload: T }) => void,
    ) => Promise<UnlistenFn>)(event, cb);
    return un;
  }
  connectWs();
  let nextId = 0;
  const h: Handler = (ev, payload) => {
    if (ev === event) {
      nextId += 1;
      cb({ event: ev, id: nextId, payload: payload as T });
    }
  };
  handlers.add(h);
  return () => {
    handlers.delete(h);
  };
}
