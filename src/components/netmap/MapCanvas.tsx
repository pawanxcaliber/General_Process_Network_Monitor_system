import {
  createEffect,
  createMemo,
  onCleanup,
  onMount,
  Show,
  type Component,
} from "solid-js";
import cytoscape, { type Core, type ElementDefinition } from "cytoscape";
import fcose from "cytoscape-fcose";
import { themeStore } from "@/stores/themeStore";
import type { MapEdge, MapNode } from "@/types";

cytoscape.use(fcose);

interface Props {
  nodes: MapNode[];
  edges: MapEdge[];
  threshold: number;
  search: string;
  autoLayout: boolean;
  selectedId: string | null;
  visibleKinds: Set<string>;
  onNodeSelect: (id: string | null) => void;
  onReady: (cy: Core) => void;
  paused?: boolean;
}

const KIND_STYLE: Record<string, { bg: string; border: string }> = {
  process: { bg: "#2c3a4f", border: "#94a3b8" },
  docker: { bg: "#14532d", border: "#22c55e" },
  podman: { bg: "#1e3a8a", border: "#3b82f6" },
  pod: { bg: "#4c1d95", border: "#a78bfa" },
  vm: { bg: "#3f3116", border: "#d97706" },
  external: { bg: "#713f12", border: "#f59e0b" },
  system: { bg: "#450a0a", border: "#f87171" },
};

function nodeRadius(rate: number): number {
  const r = 14 + 26 * Math.min(1, Math.log10(1 + rate) / 6.5);
  return Math.max(14, Math.min(40, r));
}

function edgeWidth(rate: number): number {
  return 1 + 7 * Math.min(1, Math.log10(1 + rate) / 6.5);
}

function shortLabel(s: string): string {
  return s.length > 16 ? `${s.slice(0, 13)}…` : s;
}

function fmtKbps(v: number): string {
  if (v >= 1024 * 1024) return `${(v / (1024 * 1024)).toFixed(1)} MB/s`;
  if (v >= 1024) return `${(v / 1024).toFixed(1)} KB/s`;
  return `${v.toFixed(0)} B/s`;
}

interface ThemePalette {
  label: string;
  edge: string;
  labelBg: string;
}

function readThemePalette(): ThemePalette {
  const cs = getComputedStyle(document.documentElement);
  const v = (name: string) => {
    const raw = cs.getPropertyValue(name).trim();
    const parts = raw.split(/\s+/);
    return parts.length >= 3 ? `rgb(${parts.join(",")})` : raw;
  };
  return { label: v("--ink-50"), edge: v("--edge"), labelBg: v("--s800") };
}

const FCOSE_OPTS = {
  name: "fcose",
  quality: "default",
  randomize: false,
  animate: false,
  nodeSeparation: 90,
  idealEdgeLength: () => 180,
  nodeRepulsion: () => 24000,
  padding: 40,
} as any;

export const MapCanvas: Component<Props> = (props) => {
  const palette = createMemo(() => {
    themeStore.theme();
    return readThemePalette();
  });

  let wrap!: HTMLDivElement;
  let container!: HTMLDivElement;
  let tooltip!: HTMLDivElement;
  let cy: Core | undefined;
  let fitted = false;

  onMount(() => {
    cy = cytoscape({
      container,
      elements: [],
      style: [
        {
          selector: "node",
          style: {
            "background-color": "data(bg)",
            "border-width": 2,
            "border-color": "data(border)",
            label: "data(short)",
            color: "data(tcolor)",
            "text-background-color": "data(tbg)",
            "text-background-opacity": 0.9,
            "text-background-padding": 3,
            "text-background-shape": "round-rectangle",
            "font-size": 9.5,
            "text-valign": "bottom",
            "text-margin-y": 6,
            "text-wrap": "ellipsis",
            "text-max-width": 100,
            width: "data(size)",
            height: "data(size)",
          },
        },
        { selector: "node.halo", style: { "border-width": 4, "border-color": "#22c55e" } },
        { selector: "node.muted", style: { opacity: 0.15 } },
        { selector: "node.hidden", style: { display: "none" } },
        {
          selector: "edge",
          style: {
            width: "data(width)",
            "line-color": "data(ecolor)",
            "curve-style": "bezier",
            "target-arrow-color": "data(ecolor)",
            "target-arrow-shape": "data(tarrow)",
            "arrow-scale": 1.4,
          },
        },
        { selector: "edge.muted", style: { opacity: 0.08 } },
        { selector: "edge.hidden", style: { display: "none" } },
      ] as any,
      layout: { name: "preset" },
      wheelSensitivity: 0.2,
      minZoom: 0.2,
      maxZoom: 3,
    });

    props.onReady(cy);

    const g = cy;

    g.on("tap", "node", (e) => props.onNodeSelect(e.target.id()));
    g.on("tap", (e) => {
      if (e.target === cy) props.onNodeSelect(null);
    });

    g.on("mouseover", "node", (e) => {
      const t = e.target;
      tooltip.textContent = `${t.data("full") ?? t.data("label") ?? ""}`;
      tooltip.style.display = "block";
    });
    g.on("mouseover", "edge", (e) => {
      const d = e.target.data();
      tooltip.textContent = `→ ${fmtKbps(d.tx_bps ?? 0)} · ← ${fmtKbps(d.rx_bps ?? 0)}`;
      tooltip.style.display = "block";
    });
    g.on("mousemove", (e) => {
      if (tooltip.style.display === "block") {
        tooltip.style.left = `${e.renderedPosition.x + 14}px`;
        tooltip.style.top = `${e.renderedPosition.y + 14}px`;
      }
    });
    g.on("mouseout", () => {
      tooltip.style.display = "none";
    });

    onCleanup(() => {
      cy?.destroy();
    });
  });

  const relayout = (animate: boolean) => {
    cy?.layout({
      ...FCOSE_OPTS,
      animate,
      animationDuration: 450,
    }).run();
  };

  createEffect(() => {
    sync(props.nodes, props.edges, props.threshold, props.search, props.visibleKinds);
  });

  createEffect(() => {
    const l = palette().label;
    const e = palette().edge;
    const b = palette().labelBg;
    if (!cy) return;
    const g = cy;
    g.batch(() => {
      g.nodes().forEach((n) => {
        n.data("tcolor", l);
        n.data("tbg", b);
      });
      g.edges().forEach((el) => {
        el.data("ecolor", e);
      });
    });
  });

  createEffect(() => {
    if (!cy) return;
    const g = cy;
    g.nodes().removeClass("halo").removeClass("muted");
    g.edges().removeClass("muted").removeClass("focus");
    if (props.selectedId) {
      const sel = g.getElementById(props.selectedId);
      sel.addClass("halo");
      sel.connectedEdges().addClass("focus");
      g.edges().not(sel.connectedEdges()).addClass("muted");
      g.nodes().not(sel.closedNeighborhood()).addClass("muted");
    }
  });

  function spawnPosition(nodeId: string, edges: MapEdge[]): { x: number; y: number } {
    const graph = cy;
    if (graph) {
      const neighborEdge = edges.find(
        (e) =>
          (e.source === nodeId && graph.getElementById(e.target).length > 0) ||
          (e.target === nodeId && graph.getElementById(e.source).length > 0),
      );
      if (neighborEdge) {
        const otherId =
          neighborEdge.source === nodeId ? neighborEdge.target : neighborEdge.source;
        const p = graph.getElementById(otherId).position();
        const angle = Math.random() * Math.PI * 2;
        const r = 90 + Math.random() * 60;
        return { x: p.x + Math.cos(angle) * r, y: p.y + Math.sin(angle) * r };
      }
    }
    return { x: 200 + Math.random() * 500, y: 150 + Math.random() * 350 };
  }

  function sync(
    nodes: MapNode[],
    edges: MapEdge[],
    threshold: number,
    search: string,
    visibleKinds: Set<string>,
  ) {
    const graph = cy;
    if (!graph) return;

    const nodeIds = new Set(nodes.map((n) => n.id));
    const edgeIds = new Set(edges.map((e) => e.id));
    graph.nodes().forEach((n) => {
      if (!nodeIds.has(n.id())) n.remove();
    });
    graph.edges().forEach((e) => {
      if (!edgeIds.has(e.id())) e.remove();
    });

    const q = search.trim().toLowerCase();
    const matchingIds: Set<string> | null = q
      ? new Set(
          nodes
            .filter((n) => n.label.toLowerCase().includes(q) || n.kind.includes(q))
            .map((n) => n.id),
        )
      : null;

    let added = 0;
    for (const n of nodes) {
      const kind = KIND_STYLE[n.kind] ?? KIND_STYLE.process;
      const size = nodeRadius(n.rate_bps);
      const existing = graph.getElementById(n.id);
      if (existing.empty()) {
        graph.add({
          group: "nodes",
          data: {
            id: n.id,
            short: shortLabel(n.label),
            full: n.label,
            bg: kind.bg,
            border: kind.border,
            ecolor: palette().edge,
            tcolor: palette().label,
            tbg: palette().labelBg,
            size,
            kind: n.kind,
          },
          position: spawnPosition(n.id, edges),
        } as ElementDefinition);
        added++;
      } else {
        existing.data({
          short: shortLabel(n.label),
          full: n.label,
          size,
          bg: kind.bg,
          border: kind.border,
          tcolor: palette().label,
          tbg: palette().labelBg,
        });
      }
    }

    for (const e of edges) {
      const width = edgeWidth(e.rate_bps);
      const data = {
        id: e.id,
        source: e.source,
        target: e.target,
        width,
        tx_bps: e.tx_bps,
        rx_bps: e.rx_bps,
        ports: e.ports,
        tarrow: e.tx_bps > 64 ? "triangle" : "none",
        ecolor: palette().edge,
      };
      const existing = graph.getElementById(e.id);
      if (existing.empty()) {
        graph.add({ group: "edges", data } as ElementDefinition);
        added++;
      } else {
        existing.data(data);
      }
    }

    graph.batch(() => {
      const kindVisible = (id: string) => {
        const info = nodes.find((n) => n.id === id);
        return info ? visibleKinds.has(info.kind) : true;
      };
      graph.nodes().forEach((n) => {
        const rate = nodeRateOf(nodes, n.id());
        const kindVisibleMatch = visibleKinds.has(n.data("kind") as string);
        const selectedMatch = matchingIds?.has(n.id()) ?? false;
        n.toggleClass(
          "hidden",
          !kindVisibleMatch || (rate < threshold * 0.6 && !selectedMatch),
        );
      });
      graph.edges().forEach((e) => {
        const d = e.data();
        const total = (d.tx_bps ?? 0) + (d.rx_bps ?? 0);
        const endpointsVisible = kindVisible(d.source) && kindVisible(d.target);
        const endpointsMatched =
          matchingIds === null ||
          matchingIds.has(d.source) ||
          matchingIds.has(d.target);
        e.toggleClass("hidden", !endpointsVisible || total < threshold);
        e.toggleClass("dim", !endpointsMatched);
      });
    });

    if (graph.nodes().length > 0) {
      if (!fitted) {
        relayout(false);
        graph.fit(undefined, 40);
        fitted = true;
      } else if (added > 0 && props.autoLayout) {
        relayout(false);
      }
    }
  }

  return (
    <div ref={wrap} class="relative flex-1 min-h-0">
      <div ref={container} class="absolute inset-0" />
      <Show when={props.paused}>
        <div class="absolute top-2 left-2 z-20 flex items-center gap-1 rounded border border-amber-500/40 bg-amber-500/10 px-2 py-0.5 font-mono text-[10px] text-warn">
          ❄ Data frozen
        </div>
      </Show>
      <div
        ref={tooltip}
        class="absolute z-20 pointer-events-none bg-surface-800 border border-surface-700 rounded px-2 py-1 text-[10px] font-mono text-ink-100"
        style={{ display: "none" }}
      />
    </div>
  );
};

function nodeRateOf(nodes: MapNode[], id: string): number {
  return nodes.find((n) => n.id === id)?.rate_bps ?? 0;
}
