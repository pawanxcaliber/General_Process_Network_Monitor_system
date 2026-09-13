import { Component, For } from "solid-js";

interface Props {
  threshold: number;
  onThreshold: (v: number) => void;
  search: string;
  onSearch: (v: string) => void;
  paused: boolean;
  onPaused: (v: boolean) => void;
  autoArrange: boolean;
  onAutoArrange: (v: boolean) => void;
  onFit: () => void;
  onRelayout: () => void;
  nodeCount: number;
  edgeCount: number;
  zoomPct: number;
  onZoomIn: () => void;
  onZoomOut: () => void;
  deselectedKinds: Set<string>;
  onToggleKind: (kind: string) => void;
  onAllKinds: () => void;
}

const KINDS = [
  { id: "process", label: "Process", color: "#94a3b8" },
  { id: "docker", label: "Docker", color: "#22c55e" },
  { id: "podman", label: "Podman", color: "#3b82f6" },
  { id: "pod", label: "K8s Pod", color: "#a78bfa" },
  { id: "vm", label: "VM", color: "#d97706" },
  { id: "external", label: "External", color: "#f59e0b" },
  { id: "system", label: "System (root)", color: "#f87171" },
];

export const MapControls: Component<Props> = (props) => {
  return (
    <div class="flex flex-wrap items-center gap-3 px-4 py-2 border-b border-surface-800 bg-surface-900/60">
      <div class="flex items-center gap-2">
        <span class="text-[10px] text-ink-700 uppercase tracking-wider">Min rate</span>
        <input
          type="range"
          min="0"
          max="100"
          value={props.threshold}
          onInput={(e) => props.onThreshold(Number(e.currentTarget.value))}
          class="w-32 accent-green-500"
        />
        <span class="text-[10px] font-mono text-ink-100 w-16">
          {fmtThreshold(props.threshold)}
        </span>
      </div>

      <input
        type="text"
        placeholder="Filter nodes…"
        value={props.search}
        onInput={(e) => props.onSearch(e.currentTarget.value)}
        class="bg-surface-900 border border-surface-800 rounded-md px-2.5 py-1 text-xs w-44 placeholder:text-ink-700 focus:outline-none focus:border-green-500/50"
      />

      <button
        onClick={() => props.onPaused(!props.paused)}
        class={`text-[11px] px-2.5 py-1 rounded border transition-colors ${
          props.paused
            ? "border-amber-500/40 text-warn bg-amber-500/10"
            : "border-surface-800 text-ink-700 hover:text-ink-100"
        }`}
      >
        {props.paused ? "Paused" : "Live"}
      </button>
      <button
        onClick={() => props.onAutoArrange(!props.autoArrange)}
        class={`text-[11px] px-2.5 py-1 rounded border transition-colors ${
          props.autoArrange
            ? "border-blue-500/40 text-info bg-blue-500/10"
            : "border-surface-800 text-ink-700 hover:text-ink-100"
        }`}
        title="Re-arrange layout automatically when new nodes appear"
      >
        Auto-arrange
      </button>
      <button
        onClick={props.onFit}
        class="text-[11px] px-2.5 py-1 rounded border border-surface-800 text-ink-700 hover:text-ink-100 transition-colors"
      >
        Fit
      </button>
      <button
        onClick={props.onZoomOut}
        class="text-[11px] px-2 py-1 rounded border border-surface-800 text-ink-700 hover:text-ink-100 transition-colors"
        title="Zoom out"
      >
        −
      </button>
      <span class="w-10 text-center font-mono text-[10px] text-ink-100 select-none">
        {props.zoomPct}%
      </span>
      <button
        onClick={props.onZoomIn}
        class="text-[11px] px-2 py-1 rounded border border-surface-800 text-ink-700 hover:text-ink-100 transition-colors"
        title="Zoom in"
      >
        ＋
      </button>
      <button
        onClick={props.onRelayout}
        class="text-[11px] px-2.5 py-1 rounded border border-surface-800 text-ink-700 hover:text-ink-100 transition-colors"
      >
        Re-layout
      </button>

      <span class="text-[10px] font-mono text-ink-700 ml-auto">
        {props.nodeCount} nodes · {props.edgeCount} edges
      </span>

      <div class="flex w-full items-center flex-wrap gap-1.5 py-1 border-t border-surface-800/60">
        <button
          onClick={props.onAllKinds}
          class={`text-[10px] px-2 py-0.5 rounded-full border transition-colors ${
            props.deselectedKinds.size === 0
              ? "border-green-500/50 text-ok bg-green-500/10"
              : "border-surface-700 text-ink-700 hover:text-ink-100"
          }`}
        >
          All
        </button>
        <For each={KINDS}>
          {(k) => {
            const off = () => props.deselectedKinds.has(k.id);
            return (
              <button
                onClick={() => props.onToggleKind(k.id)}
                class={`flex items-center gap-1 text-[9px] px-2 py-0.5 rounded-full border transition-colors ${
                  off()
                    ? "border-surface-800 text-ink-700 opacity-60"
                    : "border-surface-700 text-ink-100"
                }`}
              >
                <span
                  class="w-2 h-2 rounded-full"
                  style={{ "background-color": k.color, opacity: off() ? 0.35 : 1 }}
                />
                {k.label}
              </button>
            );
          }}
        </For>
      </div>
    </div>
  );
};

function fmtThreshold(pct: number): string {
  if (pct === 0) return "all";
  const bps = Math.pow(10, (pct / 100) * 7);
  if (bps >= 1024 * 1024) return `${(bps / (1024 * 1024)).toFixed(0)} MB/s`;
  if (bps >= 1024) return `${(bps / 1024).toFixed(0)} KB/s`;
  return `${bps.toFixed(0)} B/s`;
}
