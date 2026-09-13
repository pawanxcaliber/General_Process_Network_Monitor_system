import { Component, For, Show } from "solid-js";
import type { TopologyNode } from "@/types";
import { fmtBps, fmtBytes, fmtPct } from "@/lib/format";

interface NodeProps {
  node: TopologyNode;
  origin?: string;
  selected?: boolean;
  x: number;
  y: number;
  width: number;
  height: number;
}

const STATUS_COLORS: Record<string, string> = {
  running: "#22c55e",
  paused: "#f59e0b",
  exited: "#64748b",
  restarting: "#f97316",
};

const KIND_ICONS: Record<string, string> = {
  host: "⬡",
  network: "◈",
  container: "▣",
  process: "◉",
  k8s_namespace: "◫",
  k8s_pod: "◇",
  k8s_service: "△",
  vm: "▭",
};

const CARD_CLASS =
  "fill-slate-200 stroke-slate-400/60 dark:fill-slate-700 dark:stroke-slate-500/60";

const ORIGIN_COLORS: Record<string, string> = {
  docker: "#22c55e",
  podman: "#3b82f6",
  kubernetes: "#a78bfa",
  vm: "#f59e0b",
};

export const Node: Component<NodeProps> = (props) => {
  const statusColor = () => STATUS_COLORS[props.node.status] ?? "#64748b";
  const icon = () => KIND_ICONS[props.node.kind] ?? "▣";
  const isContainer = () => props.node.kind === "container";
  const isPod = () => props.node.kind === "k8s_pod";
  const isNetwork = () => props.node.kind === "network";
  const isVm = () => props.node.kind === "vm";

  const subtitle = () => {
    if (props.node.image) {
      const raw = isVm()
        ? props.node.image
        : props.node.image.split("/").pop()?.split(":")[0] ?? "";
      return raw.length > 26 ? `${raw.slice(0, 23)}…` : raw;
    }
    return props.node.kind;
  };

  return (
    <g transform={`translate(${props.x}, ${props.y})`}>
      <rect
        width={props.width}
        height={props.height}
        rx="8"
        class={CARD_CLASS}
        stroke-width="1"
      />
      <rect width="4" height={props.height} rx="2" fill={statusColor()} />
      <Show when={props.origin !== undefined && ORIGIN_COLORS[props.origin] !== undefined}>
        <rect
          x={props.width - 10}
          y="6"
          width="6"
          height="6"
          rx="2"
          fill={ORIGIN_COLORS[props.origin!]!}
        />
      </Show>

      <text x="14" y="20" class="fill-ink-50" font-size="12" font-weight="600">
        {icon()} {props.node.name}
      </text>

      <text x="14" y="34" class="fill-ink-700" font-size="9" font-family="monospace">
        {subtitle()}
      </text>

      <Show when={isContainer()}>
        <text x="14" y="50" class="fill-ink-700" font-size="9" font-family="monospace">
          CPU {fmtPct(props.node.metrics.cpu_percent, 0)} · MEM{" "}
          {fmtBytes(props.node.metrics.memory_bytes, 0)}
        </text>
        <text x="14" y="62" class="fill-ink-800" font-size="9" font-family="monospace">
          ↓ {fmtBps(props.node.metrics.rx_rate_bps)} · ↑{" "}
          {fmtBps(props.node.metrics.tx_rate_bps)}
        </text>
      </Show>

      <Show when={isPod()}>
        <text x="14" y="50" class="fill-ink-700" font-size="9" font-family="monospace">
          CPU {fmtPct(props.node.metrics.cpu_percent, 0)} · MEM{" "}
          {fmtBytes(props.node.metrics.memory_bytes, 0)}
        </text>
      </Show>

      <Show when={isNetwork()}>
        <text x="14" y="50" class="fill-ink-800" font-size="9" font-family="monospace">
          ↓ {fmtBps(props.node.metrics.rx_rate_bps)} aggregate
        </text>
      </Show>

      <Show when={isVm()}>
        <text x="14" y="50" class="fill-ink-700" font-size="9" font-family="monospace">
          {props.node.metrics.memory_bytes > 0
            ? `${fmtBytes(props.node.metrics.memory_bytes, 0)} allocated`
            : "—"}
        </text>
      </Show>

      <For each={props.node.ports.slice(0, 3)}>
        {(port, i) => (
          <g>
            <rect
              x={14 + i() * 62}
              y={props.height - 30}
              width="56"
              height="16"
              rx="8"
              fill={port.host_port ? "#22c55e18" : "#3b82f618"}
              stroke={port.host_port ? "#22c55e44" : "#3b82f644"}
              stroke-width="0.5"
            />
            <text
              x={14 + i() * 62 + 28}
              y={props.height - 19}
              text-anchor="middle"
              class={port.host_port ? "fill-ok" : "fill-info"}
              font-size="9"
              font-family="monospace"
            >
              {port.host_port
                ? `${port.host_port}:${port.container_port}`
                : `${port.container_port}/${port.proto}`}
            </text>
          </g>
        )}
      </For>
      {props.node.ports.length > 3 && (
        <text
          x={14 + 3 * 62 + 4}
          y={props.height - 19}
          class="fill-ink-800"
          font-size="9"
          font-family="monospace"
        >
          +{props.node.ports.length - 3}
        </text>
      )}
    </g>
  );
};
