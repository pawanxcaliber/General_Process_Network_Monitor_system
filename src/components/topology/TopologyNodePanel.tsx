import { Component, For, Show, createMemo, createSignal } from "solid-js";
import { A } from "@solidjs/router";
import type { TopologyEdge, TopologyNode } from "@/types";
import { fmtBps, fmtBytes, fmtPct } from "@/lib/format";

interface Props {
  node: TopologyNode;
  edges: TopologyEdge[];
  nodes: TopologyNode[];
  origin?: string;
  onClose: () => void;
}

const KIND_LABEL: Record<string, string> = {
  host: "Host machine",
  network: "Container network",
  container: "Container",
  process: "Process",
  k8s_namespace: "Kubernetes namespace",
  k8s_pod: "Kubernetes pod",
  k8s_service: "Kubernetes service",
  vm: "Virtual machine",
};

const KIND_COLOR: Record<string, string> = {
  host: "#22c55e",
  network: "#3b82f6",
  container: "#94a3b8",
  process: "#94a3b8",
  k8s_namespace: "#a78bfa",
  k8s_pod: "#a78bfa",
  k8s_service: "#3b82f6",
  vm: "#f59e0b",
};

const ORIGIN_LABEL: Record<string, string> = {
  docker: "Docker",
  podman: "Podman",
  kubernetes: "Kubernetes",
  vm: "VM",
  apps: "Apps",
  system: "System (root)",
};

export const TopologyNodePanel: Component<Props> = (props) => {
  const [tab, setTab] = createSignal<"summary" | "network">("summary");
  const node = () => props.node;

  const peers = createMemo(() =>
    props.edges
      .filter((e) => e.source === node().id || e.target === node().id)
      .map((e) => {
        const outbound = e.source === node().id;
        const peerId = e.source === node().id ? e.target : e.source;
        const peer = props.nodes.find((n) => n.id === peerId);
        return {
          id: peerId,
          name: peer?.name ?? peerId,
          kind: peer?.kind ?? "container",
          origin: peer?.kind ? undefined : undefined,
          traffic: e.traffic_bps,
          label: e.label,
          outbound,
          edgeId: e.id,
        };
      })
      .sort((a, b) => b.traffic - a.traffic),
  );

  const totalTraffic = createMemo(() =>
    peers().reduce((acc, p) => acc + p.traffic, 0),
  );

  const containerLink = () => {
    const o = props.origin;
    const id = node().id;
    if (o === "docker" && id.startsWith("ctr:"))
      return `/docker/containers/${id.slice("ctr:".length)}`;
    if (o === "podman" && id.startsWith("ctr:"))
      return `/podman/containers/${id.slice("ctr:".length)}`;
    return null;
  };

  const detailLabel = () => {
    const kind = node().kind;
    if (kind === "process") return "Command line";
    if (kind === "container" || kind === "vm") return "Image";
    return "Detail";
  };

  return (
    <div class="w-80 shrink-0 border-l border-surface-800 bg-surface-900 flex flex-col overflow-y-auto">
      <div class="flex items-start justify-between px-4 pt-3 pb-2">
        <div class="min-w-0">
          <div class="flex items-center gap-2">
            <span
              class="w-2.5 h-2.5 rounded-full shrink-0"
              style={{ "background-color": KIND_COLOR[node().kind] ?? "#94a3b8" }}
            />
            <h3 class="text-sm font-semibold truncate">{node().name}</h3>
          </div>
          <div class="flex items-center gap-2 mt-0.5">
            <p class="text-[10px] text-ink-700">{KIND_LABEL[node().kind] ?? node().kind}</p>
            <Show when={props.origin && ORIGIN_LABEL[props.origin]}>
              <span class="text-[9px] px-1.5 rounded-full border border-surface-700 text-ink-100">
                {ORIGIN_LABEL[props.origin!]}
              </span>
            </Show>
          </div>
        </div>
        <button
          onClick={props.onClose}
          class="text-ink-700 hover:text-ink-100 text-sm px-1"
        >
          ✕
        </button>
      </div>

      <div class="flex gap-1 px-4 pb-2">
        <TabBtn active={tab() === "summary"} onClick={() => setTab("summary")} label="Summary" />
        <TabBtn
          active={tab() === "network"}
          onClick={() => setTab("network")}
          label={`Network (${peers().length})`}
        />
      </div>

      <Show when={tab() === "summary"}>
        <div class="px-4 pb-4 space-y-3 text-xs">
          <Show when={node().image}>
            <div>
              <span class="text-ink-700 uppercase tracking-wider text-[9px] block mb-1">
                {detailLabel()}
              </span>
              <p class="font-mono text-[11px] text-ink-100 break-all">{node().image}</p>
            </div>
          </Show>

          <div class="grid grid-cols-2 gap-2">
            <Stat label="CPU" value={fmtPct(node().metrics.cpu_percent, 1)} color="text-ink-100" />
            <Stat
              label="Memory"
              value={fmtBytes(node().metrics.memory_bytes)}
              color="text-ink-100"
            />
            <Stat label="↓ RX" value={fmtBps(node().metrics.rx_rate_bps)} color="text-ok" />
            <Stat label="↑ TX" value={fmtBps(node().metrics.tx_rate_bps)} color="text-info" />
            <Stat label="Total traffic" value={fmtBps(totalTraffic())} color="text-ink-100" />
            <Stat
              label="Connections"
              value={String(peers().length)}
              color="text-ink-100"
            />
          </div>

          <Show when={node().ports.length > 0}>
            <div>
              <span class="text-ink-700 uppercase tracking-wider text-[9px] block mb-1">
                Ports
              </span>
              <div class="flex flex-wrap gap-1">
                <For each={node().ports}>
                  {(p) => (
                    <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface-800 text-ink-100 border border-surface-700">
                      {p.host_port
                        ? `${p.host_port}:${p.container_port}`
                        : `${p.container_port}/${p.proto}`}
                    </span>
                  )}
                </For>
              </div>
            </div>
          </Show>

          <Show when={containerLink()}>
            {(href) => (
              <A
                href={href()}
                class="block text-center text-[11px] px-3 py-1.5 rounded border border-surface-800 text-ink-100 hover:border-green-500/40 hover:text-ok transition-colors"
              >
                Open container view →
              </A>
            )}
          </Show>
        </div>
      </Show>

      <Show when={tab() === "network"}>
        <div class="px-4 pb-4 space-y-2">
          <For each={peers()}>
            {(p) => (
              <div class="bg-surface-950/60 border border-surface-800 rounded-md px-2.5 py-2">
                <div class="flex items-center gap-2">
                  <span
                    class="w-1.5 h-1.5 rounded-full shrink-0"
                    style={{ "background-color": KIND_COLOR[p.kind] ?? "#94a3b8" }}
                  />
                  <span class="text-[11px] font-medium truncate" title={p.name}>
                    {p.name}
                  </span>
                  <span class="text-[9px] text-ink-700 ml-auto shrink-0">
                    {p.outbound ? "→" : "←"}
                  </span>
                </div>
                <div class="flex items-center gap-2 text-[10px] font-mono mt-1.5">
                  <span class={p.traffic > 0 ? "text-ok" : "text-ink-700"}>
                    {fmtBps(p.traffic)}
                  </span>
                  <Show when={p.label}>
                    <span class="text-ink-700 ml-auto truncate">{p.label}</span>
                  </Show>
                </div>
              </div>
            )}
          </For>
          {peers().length === 0 && (
            <div class="text-ink-700 text-center py-8 text-xs">No connections</div>
          )}
        </div>
      </Show>
    </div>
  );
};

const TabBtn: Component<{ active: boolean; onClick: () => void; label: string }> = (props) => (
  <button
    onClick={props.onClick}
    class={`px-3 py-1 text-[11px] rounded-md transition-colors ${
      props.active ? "bg-surface-800 text-ink-50" : "text-ink-700 hover:text-ink-100"
    }`}
  >
    {props.label}
  </button>
);

const Stat: Component<{ label: string; value: string; color: string }> = (props) => (
  <div class="bg-surface-950/60 border border-surface-800 rounded-md px-2.5 py-2">
    <span class="text-[9px] text-ink-700 uppercase tracking-wider">{props.label}</span>
    <div class={`text-sm font-mono font-semibold ${props.color}`}>{props.value}</div>
  </div>
);
