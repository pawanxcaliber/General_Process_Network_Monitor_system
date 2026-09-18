import { Component, For, Show, createMemo, createSignal } from "solid-js";
import { runtimeStore } from "@/stores/runtimeStore";
import { networkMapStore } from "@/stores/networkMapStore";
import { hostStore } from "@/stores/hostStore";
import { TopologyCanvas } from "@/components/topology/TopologyCanvas";
import { TopologyNodePanel } from "@/components/topology/TopologyNodePanel";
import type {
  NetworkMapPayload,
  NodeMetrics,
  ProcessInfo,
  RuntimeKind,
  RuntimeSnapshot,
  TopologyEdge,
  TopologyNode,
} from "@/types";

const RUNTIMES: RuntimeKind[] = ["docker", "podman", "kubernetes", "vm"];

const RUNTIME_LABELS: Record<string, string> = {
  docker: "Docker",
  podman: "Podman",
  kubernetes: "Kubernetes",
  vm: "VMs",
};

export const ORIGIN_COLORS: Record<string, string> = {
  docker: "#22c55e",
  podman: "#3b82f6",
  kubernetes: "#a78bfa",
  vm: "#f59e0b",
};

interface Merged {
  nodes: TopologyNode[];
  edges: TopologyEdge[];
  origin: Map<string, string>;
  unavailable: string[];
  hostIndex: number;
}

function portKey(p: { host_port?: number; container_port: number; proto: string }): string {
  return `${p.host_port ?? ""}:${p.container_port}:${p.proto}`;
}

function sumMetrics(a: NodeMetrics, b: NodeMetrics): NodeMetrics {
  return {
    cpu_percent: a.cpu_percent + b.cpu_percent,
    memory_bytes: a.memory_bytes + b.memory_bytes,
    rx_rate_bps: a.rx_rate_bps + b.rx_rate_bps,
    tx_rate_bps: a.tx_rate_bps + b.tx_rate_bps,
  };
}

function mergeTopology(
  runtimes: RuntimeKind[],
  nmParam?: NetworkMapPayload | null,
): Merged {
  const nodes: TopologyNode[] = [];
  const edges: TopologyEdge[] = [];
  const origin = new Map<string, string>();
  const portIndex = new Map<string, Set<string>>();
  let hostIndex: number | null = null;
  const unavailable: string[] = [];

  for (const rt of runtimes) {
    if (!runtimeStore.has(rt)) {
      unavailable.push(rt);
      continue;
    }
    const s: RuntimeSnapshot = runtimeStore.snap(rt);
    const netPrefix = (id: string) =>
      rt === "docker" || rt === "podman" ? id.replace(/^net:/, `net:${rt}:`) : id;

    for (const n of s.nodes) {
      if (n.id === "host") {
        if (hostIndex === null) {
          hostIndex = nodes.length;
          nodes.push({ ...n });
          portIndex.set(
            "host",
            new Set(n.ports.map(portKey)),
          );
        } else {
          const h = nodes[hostIndex];
          h.metrics = sumMetrics(h.metrics, n.metrics);
          const seen = portIndex.get("host")!;
          for (const p of n.ports) {
            const key = portKey(p);
            if (!seen.has(key)) {
              seen.add(key);
              h.ports.push(p);
            }
          }
        }
        continue;
      }
      if ((rt === "docker" || rt === "podman") && n.id.startsWith("net:")) {
        const id = netPrefix(n.id);
        nodes.push({ ...n, id, parent_id: n.parent_id ? netPrefix(n.parent_id) : undefined });
        origin.set(id, rt);
      } else {
        nodes.push(n);
        origin.set(n.id, rt);
      }
    }

    for (const e of s.edges) {
      const source = netPrefix(e.source);
      const target = e.target;
      edges.push({
        ...e,
        id: `${rt}:${e.id}`,
        source,
        target,
      });
    }
  }

  if (hostIndex === null) {
    hostIndex = nodes.length;
    nodes.push({
      id: "host",
      name: "Host",
      kind: "host",
      status: "running",
      metrics: { cpu_percent: 0, memory_bytes: 0, rx_rate_bps: 0, tx_rate_bps: 0 },
      ports: [],
    });
  }

  const nm = nmParam ?? networkMapStore.map();
  if (nm) {
    const h = hostStore.host();
    const procByPid = new Map<number, ProcessInfo>();
    for (const p of h?.processes ?? []) procByPid.set(p.pid, p);
    for (const n of nm.nodes) {
      if (n.kind !== "process" && n.kind !== "system") continue;
      const key = n.kind === "system" ? "system" : "apps";
      origin.set(n.id, key);
      const pid = n.id.startsWith("proc:") ? Number(n.id.slice(5)) : null;
      const proc = pid != null ? procByPid.get(pid) : undefined;
      nodes.push({
        id: n.id,
        name: n.label,
        kind: "process",
        status: "running",
        image: n.detail ?? undefined,
        metrics: {
          cpu_percent:
            proc?.cpu_percent ??
            (n.kind === "system" ? (h?.cpu_percent ?? 0) : 0),
          memory_bytes: proc?.memory_bytes ?? (n.kind === "system" ? (h?.ram_used_bytes ?? 0) : 0),
          rx_rate_bps: n.rx_bps,
          tx_rate_bps: n.tx_bps,
        },
        ports: n.listeners.slice(0, 6).map((p) => ({ container_port: p, proto: "tcp" })),
      });
      edges.push({
        id: `e:pmap:${n.id}`,
        source: "host",
        target: n.id,
        traffic_bps: n.rate_bps,
      });
    }
  }

  return { nodes, edges, origin, unavailable, hostIndex: hostIndex ?? 0 };
}

const GROUPS_STORAGE_KEY = "monitor-map-groups";

function loadShowGroups(): boolean {
  try {
    return localStorage.getItem(GROUPS_STORAGE_KEY) !== "0";
  } catch {
    return true;
  }
}

function saveShowGroups(v: boolean) {
  try {
    localStorage.setItem(GROUPS_STORAGE_KEY, v ? "1" : "0");
  } catch {
    // ignore
  }
}

export const AllMap: Component<{
  nm?: NetworkMapPayload | null;
  paused?: boolean;
  onPaused?: (v: boolean) => void;
}> = (props) => {
  const merged = createMemo(() => mergeTopology([...RUNTIMES], props.nm));
  const empty = createMemo(() => RUNTIMES.every((rt) => !runtimeStore.has(rt)));
  const [showGroups, setShowGroups] = createSignal(loadShowGroups());
  const [selected, setSelected] = createSignal<string | null>(null);

  const selectedNode = createMemo(() => merged().nodes.find((n) => n.id === selected()));

  const toggleGroups = () => {
    const v = !showGroups();
    setShowGroups(v);
    saveShowGroups(v);
  };

  return (
    <div class="h-full flex flex-col">
      <div class="px-5 pt-4 pb-2 flex items-baseline gap-3">
        <h2 class="text-lg font-semibold">All Runtimes Map</h2>
        <p class="text-xs text-ink-700">Docker · Podman · Kubernetes · VMs</p>
        <div class="ml-auto flex items-center gap-2">
          <Show when={props.paused}>
            <span class="text-[10px] font-mono text-warn self-center">❄ Data frozen</span>
          </Show>
          <Show when={props.onPaused}>
            <button
              onClick={() => props.onPaused?.(!props.paused)}
              class={`text-[11px] px-2.5 py-0.5 rounded border transition-colors ${
                props.paused
                  ? "border-amber-500/40 text-warn bg-amber-500/10"
                  : "border-surface-800 text-ink-700 hover:text-ink-100"
              }`}
            >
              {props.paused ? "Resume" : "Freeze"}
            </button>
          </Show>
          <span class="text-[10px] text-ink-700">
            {merged().nodes.length} nodes · {merged().edges.length} edges
          </span>
          <button
            onClick={toggleGroups}
            class={`text-[11px] px-2.5 py-0.5 rounded border transition-colors ${
              showGroups()
                ? "border-blue-500/40 text-info bg-blue-500/10"
                : "border-surface-800 text-ink-700 hover:text-ink-100"
            }`}
            title="Show runtime cluster backdrops"
          >
            Groups
          </button>
        </div>
      </div>
      <div class="flex-1 min-h-0 flex">
        <div class="flex-1 min-h-0 flex flex-col">
          <Show
            when={!empty()}
            fallback={
              <div class="h-full flex items-center justify-center text-xs text-ink-700">
                Waiting for runtime data…
              </div>
            }
          >
            <TopologyCanvas
              nodes={merged().nodes}
              edges={merged().edges}
              nodeOrigin={merged().origin}
              showGroups={showGroups()}
              selectedId={selected()}
              onSelect={setSelected}
              paused={props.paused}
            />
          </Show>
        </div>
        <Show when={selectedNode()}>
          {(n) => (
            <TopologyNodePanel
              node={n()}
              edges={merged().edges}
              nodes={merged().nodes}
              origin={merged().origin.get(n().id)}
              onClose={() => setSelected(null)}
            />
          )}
        </Show>
      </div>
      <Show when={merged().unavailable.length > 0}>
        <div class="px-5 pb-3 pt-1 flex items-center gap-2 text-[10px] text-ink-700">
          <span>Not connected:</span>
          <For each={merged().unavailable}>
            {(rt) => (
              <span class="px-1.5 py-0.5 rounded border border-surface-700 flex items-center gap-1">
                <span
                  class="w-1.5 h-1.5 rounded-full"
                  style={{ "background-color": ORIGIN_COLORS[rt] ?? "#94a3b8" }}
                />
                {RUNTIME_LABELS[rt] ?? rt}
              </span>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};
