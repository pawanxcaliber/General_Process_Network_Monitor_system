import { Component, For, Show, createSignal } from "solid-js";
import { A } from "@solidjs/router";
import type { MapEdge, MapNode, MapNodeKind } from "@/types";
import { fmtBps } from "@/lib/format";
import { portLabel } from "@/lib/services";

interface Props {
  node: MapNode;
  edges: MapEdge[];
  nodes: MapNode[];
  onClose: () => void;
}

const KIND_LABEL: Record<MapNodeKind, string> = {
  process: "Process",
  docker: "Docker container",
  podman: "Podman container",
  pod: "K8s Pod",
  vm: "Virtual machine",
  external: "External endpoint",
  system: "System (root-owned)",
};

const KIND_COLOR: Record<MapNodeKind, string> = {
  process: "#94a3b8",
  docker: "#22c55e",
  podman: "#3b82f6",
  pod: "#a78bfa",
  vm: "#d97706",
  external: "#f59e0b",
  system: "#f87171",
};

export const NodePanel: Component<Props> = (props) => {
  const [tab, setTab] = createSignal<"summary" | "network">("summary");
  const node = () => props.node;
  const conns = () =>
    props.edges
      .filter((e) => e.source === node().id || e.target === node().id)
      .map((e) => {
        const outgoing = e.source === node().id;
        const peerId = outgoing ? e.target : e.source;
        const peer = props.nodes.find((n) => n.id === peerId);
        return {
          peer: peer?.label ?? peerId,
          kind: peer?.kind ?? "process",
          sent: outgoing ? e.tx_bps : e.rx_bps,
          recv: outgoing ? e.rx_bps : e.tx_bps,
          ports: e.ports,
          connCount: e.conn_count,
          edgeId: e.id,
        };
      })
      .sort((a, b) => b.sent + b.recv - (a.sent + a.recv));

  const containerLink = () => {
    const id = node().id;
    if (id.startsWith("ctr:docker:")) return `/docker/containers/${id.slice("ctr:docker:".length)}`;
    if (id.startsWith("ctr:podman:")) return `/podman/containers/${id.slice("ctr:podman:".length)}`;
    return null;
  };

  return (
    <div class="w-80 shrink-0 border-l border-surface-800 bg-surface-900 flex flex-col overflow-y-auto">
      <div class="flex items-start justify-between px-4 pt-3 pb-2">
        <div class="min-w-0">
          <div class="flex items-center gap-2">
            <span
              class="w-2.5 h-2.5 rounded-full shrink-0"
              style={{ "background-color": KIND_COLOR[node().kind] }}
            />
            <h3 class="text-sm font-semibold truncate">{node().label}</h3>
          </div>
          <p class="text-[10px] text-ink-700 mt-0.5">{KIND_LABEL[node().kind]}</p>
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
        <TabBtn active={tab() === "network"} onClick={() => setTab("network")} label={`Network (${conns().length})`} />
      </div>

      <Show when={tab() === "summary"}>
        <div class="px-4 pb-4 space-y-3 text-xs">
          <Show when={node().detail}>
            <div>
              <span class="text-ink-700 uppercase tracking-wider text-[9px] block mb-1">
                Detail
              </span>
              <p class="font-mono text-[11px] text-ink-100 break-all">{node().detail}</p>
            </div>
          </Show>

          <div class="grid grid-cols-2 gap-2">
            <Stat label="↓ RX" value={fmtBps(node().rx_bps)} color="text-ok" />
            <Stat label="↑ TX" value={fmtBps(node().tx_bps)} color="text-info" />
            <Stat label="Total" value={fmtBps(node().rate_bps)} color="text-ink-100" />
            <Stat label="Connections" value={String(node().conn_count)} color="text-ink-100" />
          </div>

          <Show when={node().listeners.length > 0}>
            <div>
              <span class="text-ink-700 uppercase tracking-wider text-[9px] block mb-1">
                Listening on
              </span>
              <div class="flex flex-wrap gap-1">
                <For each={node().listeners}>
                  {(p) => (
                    <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-green-500/10 text-ok border border-green-500/20">
                      :{portLabel(p)}
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
          <For each={conns()}>
            {(c) => (
              <div class="bg-surface-950/60 border border-surface-800 rounded-md p-2.5">
                <div class="flex items-center gap-2">
                  <span
                    class="w-1.5 h-1.5 rounded-full shrink-0"
                    style={{ "background-color": KIND_COLOR[c.kind as MapNodeKind] ?? KIND_COLOR.process }}
                  />
                  <span class="text-[11px] font-medium truncate">{c.peer}</span>
                  <span class="text-[9px] text-ink-700 ml-auto shrink-0">
                    ×{c.connCount}
                  </span>
                </div>
                <div class="flex items-center gap-3 text-[10px] font-mono mt-1.5">
                  <span class="text-info">↑ {fmtBps(c.sent)}</span>
                  <span class="text-ok">↓ {fmtBps(c.recv)}</span>
                </div>
                <Show when={c.ports.length > 0}>
                  <div class="flex flex-wrap gap-1 mt-1.5">
                    <For each={c.ports.slice(0, 4)}>
                      {(p) => (
                        <span class="text-[9px] font-mono px-1 py-0.5 rounded bg-surface-800 text-ink-700">
                          :{portLabel(p)}
                        </span>
                      )}
                    </For>
                  </div>
                </Show>
              </div>
            )}
          </For>
          {conns().length === 0 && (
            <div class="text-center text-ink-700 py-8 text-xs">
              No active connections
            </div>
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
      props.active
        ? "bg-surface-800 text-ink-50"
        : "text-ink-700 hover:text-ink-100"
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
