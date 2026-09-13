import { onMount, onCleanup } from "solid-js";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { hostStore, runtimeStore, engineStore, networkMapStore } from "@/stores";
import type {
  HostPayload,
  NetworkMapPayload,
  RuntimeTick,
  RuntimeKind,
  RuntimeSnapshot,
  RuntimeStatus,
} from "@/types";

const KINDS: RuntimeKind[] = ["docker", "podman", "kubernetes", "vm"];

export function useBackend() {
  let unlistenHost: UnlistenFn | undefined;
  let unlistenRuntime: UnlistenFn | undefined;
  let unlistenMap: UnlistenFn | undefined;

  onMount(async () => {
    try {
      unlistenHost = await listen<HostPayload>("host-metrics-tick", (e) => {
        hostStore.setHost(e.payload);
      });
    } catch (err) {
      console.error("listen host-metrics-tick failed:", err);
    }

    try {
      unlistenRuntime = await listen<RuntimeTick>("runtime-tick", (e) => {
        runtimeStore.set(e.payload.kind, e.payload.snapshot);
      });
    } catch (err) {
      console.error("listen runtime-tick failed:", err);
    }

    try {
      unlistenMap = await listen<NetworkMapPayload>("network-map-tick", (e) => {
        networkMapStore.setMap(e.payload);
      });
    } catch (err) {
      console.error("listen network-map-tick failed:", err);
    }

    try {
      const h = await invoke<HostPayload>("get_host_metrics");
      hostStore.setHost(h);
    } catch (err) {
      console.error("get_host_metrics failed:", err);
    }

    try {
      const m = await invoke<NetworkMapPayload>("get_network_map");
      networkMapStore.setMap(m);
    } catch (err) {
      console.error("get_network_map failed:", err);
    }

    for (const kind of KINDS) {
      try {
        const snap = await invoke<RuntimeSnapshot>("get_runtime_snapshot", { kind });
        runtimeStore.set(kind, snap);
      } catch {
        console.debug("no snapshot yet for", kind);
      }
    }

    try {
      const r = await invoke<RuntimeStatus[]>("detect_runtimes");
      engineStore.setRuntimes(r);
    } catch (err) {
      console.error("detect_runtimes failed:", err);
    }
  });

  onCleanup(() => {
    unlistenHost?.();
    unlistenRuntime?.();
    unlistenMap?.();
  });
}
