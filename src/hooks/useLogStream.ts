import { createSignal, onMount, onCleanup } from "solid-js";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { logsStore } from "@/stores";
import type { LogLine, RuntimeKind } from "@/types";

export function useLogStream(kind: RuntimeKind, containerId: string) {
  let unlisten: UnlistenFn | undefined;
  const [streaming, setStreaming] = createSignal(false);

  onMount(async () => {
    const event = `container-logs:${kind}:${containerId}`;
    try {
      unlisten = await listen<LogLine>(event, (e) => {
        const line = e.payload;
        if (line.stream === "status" && line.text === "__EOF__") {
          setStreaming(false);
          return;
        }
        logsStore.append(event, line);
        setStreaming(true);
      });
    } catch (err) {
      console.error("listen container logs failed:", err);
    }

    try {
      await invoke("start_log_stream", { kind, containerId });
    } catch (err) {
      console.error("start_log_stream failed:", err);
    }
  });

  onCleanup(() => {
    unlisten?.();
    invoke("stop_log_stream", { kind, containerId }).catch(() => {});
  });

  return { streaming };
}
