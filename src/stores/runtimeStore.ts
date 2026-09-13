import { createSignal } from "solid-js";
import type { RuntimeKind, RuntimeSnapshot } from "@/types";

const [snapshots, setSnapshots] = createSignal<Partial<Record<RuntimeKind, RuntimeSnapshot>>>({});

export const runtimeStore = {
  snapshots,
  snap(kind: RuntimeKind): RuntimeSnapshot {
    return snapshots()[kind] ?? emptySnap(kind);
  },
  set(kind: RuntimeKind, snapshot: RuntimeSnapshot) {
    setSnapshots((prev) => ({ ...prev, [kind]: snapshot }));
  },
  has(kind: RuntimeKind): boolean {
    return snapshots()[kind] !== undefined;
  },
};

export function emptySnap(kind: RuntimeKind): RuntimeSnapshot {
  return {
    kind,
    available: false,
    containers: [],
    networks: [],
    nodes: [],
    edges: [],
    pods: [],
    services: [],
    vms: [],
    metrics_available: false,
    history: { cpu: [], mem_bytes: [], net_rx_bps: [], net_tx_bps: [] },
  };
}
