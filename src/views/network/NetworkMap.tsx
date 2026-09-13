import { Component, Show, createMemo, createSignal } from "solid-js";
import { networkMapStore } from "@/stores/networkMapStore";
import { MapCanvas } from "@/components/netmap/MapCanvas";
import { MapControls } from "@/components/netmap/MapControls";
import { NodePanel } from "@/components/netmap/NodePanel";
import { AllMap } from "@/views/network/AllMap";
import type { Core } from "cytoscape";
import type { NetworkMapPayload } from "@/types";

const KIND_STORAGE_KEY = "monitor-map-kinds";

function loadDeselectedKinds(): Set<string> {
  try {
    const raw = localStorage.getItem(KIND_STORAGE_KEY);
    if (!raw) return new Set();
    const arr = JSON.parse(raw);
    if (!Array.isArray(arr)) return new Set();
    return new Set(arr.filter((k: unknown) => typeof k === "string"));
  } catch {
    return new Set();
  }
}

function saveDeselectedKinds(kinds: Set<string>) {
  try {
    localStorage.setItem(KIND_STORAGE_KEY, JSON.stringify([...kinds]));
  } catch {
    // ignore
  }
}

export const NetworkMap: Component = () => {
  let cy: Core | undefined;
  const [selected, setSelected] = createSignal<string | null>(null);
  const [threshold, setThreshold] = createSignal(0);
  const [search, setSearch] = createSignal("");
  const [paused, setPaused] = createSignal(false);
  const [autoArrange, setAutoArrange] = createSignal(false);
  const [deselected, setDeselected] = createSignal<Set<string>>(loadDeselectedKinds());
  const [tab, setTab] = createSignal<"live" | "all">("live");
  const [zoomPct, setZoomPct] = createSignal(100);
  let frozen: NetworkMapPayload | null = null;

  const visibleKinds = createMemo(() => {
    const off = deselected();
    return new Set(
      ["process", "docker", "podman", "pod", "vm", "external", "system"].filter(
        (k) => !off.has(k),
      ),
    );
  });

  const toggleKind = (kind: string) => {
    const off = new Set(deselected());
    if (off.has(kind)) {
      off.delete(kind);
    } else {
      off.add(kind);
    }
    setDeselected(off);
    saveDeselectedKinds(off);
  };

  const allKinds = () => {
    const off = new Set<string>();
    setDeselected(off);
    saveDeselectedKinds(off);
  };

  const zoomBy = (f: number) => {
    if (!cy) return;
    cy.zoom({
      level: cy.zoom() * f,
      renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 },
    });
  };

  const onZoomIn = () => zoomBy(1.25);
  const onZoomOut = () => zoomBy(1 / 1.25);

  const payload = createMemo(() => {
    const p = networkMapStore.map();
    if (!paused() && p) {
      frozen = p;
    }
    return paused() ? frozen : p;
  });

  const nodes = createMemo(() => payload()?.nodes ?? []);
  const edges = createMemo(() => payload()?.edges ?? []);
  const thresholdBps = createMemo(() =>
    threshold() === 0 ? 0 : Math.pow(10, (threshold() / 100) * 7),
  );

  const selectedNode = createMemo(() => nodes().find((n) => n.id === selected()));

  return (
    <div class="h-full flex flex-col">
      <div class="px-5 pt-4 pb-2 flex items-baseline gap-4">
        <h2 class="text-lg font-semibold">Network Map</h2>
        <div class="flex items-center gap-1">
          <button
            onClick={() => setTab("live")}
            class={`text-[11px] px-2.5 py-0.5 rounded-full border transition-colors ${
              tab() === "live"
                ? "border-green-500/50 text-ok bg-green-500/10"
                : "border-surface-800 text-ink-700 hover:text-ink-100"
            }`}
          >
            Live Traffic
          </button>
          <button
            onClick={() => setTab("all")}
            class={`text-[11px] px-2.5 py-0.5 rounded-full border transition-colors ${
              tab() === "all"
                ? "border-green-500/50 text-ok bg-green-500/10"
                : "border-surface-800 text-ink-700 hover:text-ink-100"
            }`}
          >
            All Topology
          </button>
        </div>
      </div>

      <Show
        when={tab() === "live"}
        fallback={<AllMap nm={payload()} paused={paused()} onPaused={setPaused} />}
      >
        <p class="px-5 pb-2 text-xs text-ink-700">
          Live TCP traffic between processes, containers and the outside world
        </p>

        <MapControls
        threshold={threshold()}
        onThreshold={setThreshold}
        search={search()}
        onSearch={setSearch}
        paused={paused()}
        onPaused={setPaused}
        autoArrange={autoArrange()}
        onAutoArrange={setAutoArrange}
        onFit={() => cy?.fit(undefined, 40)}
        onRelayout={() =>
          cy?.layout({
            name: "fcose",
            quality: "default",
            randomize: false,
            animate: true,
            animationDuration: 450,
            nodeSeparation: 90,
            idealEdgeLength: () => 180,
            nodeRepulsion: () => 24000,
            padding: 40,
          } as any).run()
        }
        nodeCount={nodes().length}
        edgeCount={edges().length}
        zoomPct={zoomPct()}
        onZoomIn={onZoomIn}
        onZoomOut={onZoomOut}
        deselectedKinds={deselected()}
        onToggleKind={toggleKind}
        onAllKinds={allKinds}
      />

        <div class="flex-1 flex min-h-0">
        <MapCanvas
          nodes={nodes()}
          edges={edges()}
          threshold={thresholdBps()}
          search={search()}
          autoLayout={autoArrange()}
          selectedId={selected()}
          visibleKinds={visibleKinds()}
          onNodeSelect={setSelected}
          onReady={(c) => {
            cy = c;
            c.on("zoom", () => setZoomPct(Math.round(c.zoom() * 100)));
            setZoomPct(Math.round(c.zoom() * 100));
          }}
          paused={paused()}
        />
        <Show when={selectedNode()}>
          {(n) => (
            <NodePanel
              node={n()}
              edges={edges()}
              nodes={nodes()}
              onClose={() => setSelected(null)}
            />
          )}
        </Show>
      </div>

      <Show when={nodes().length === 0}>
        <div class="px-5 pb-4 text-xs text-ink-700">
          Waiting for connections… open a website, start a container, or run
          <code class="mx-1 font-mono text-ok">curl google.com</code>
          to see traffic appear here.
        </div>
      </Show>
      </Show>
    </div>
  );
};
