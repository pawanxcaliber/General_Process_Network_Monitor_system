import {
  Component,
  For,
  Show,
  createSignal,
  createMemo,
  onMount,
  onCleanup,
  createEffect,
  untrack,
} from "solid-js";
import { Node } from "./Node";
import { Edge } from "./Edge";
import type { TopologyNode, TopologyEdge } from "@/types";
import ELK from "elkjs/lib/elk.bundled.js";

const elk = new ELK();

interface Pos {
  x: number;
  y: number;
  w: number;
  h: number;
}

const NODE_W = 220;
const CLUSTER_PAD = 30;
const MIN_ZOOM = 0.15;
const MAX_ZOOM = 4;

function nodeHeight(kind: string): number {
  switch (kind) {
    case "host":
      return 96;
    case "network":
      return 70;
    case "k8s_namespace":
      return 70;
    case "k8s_service":
      return 80;
    case "k8s_pod":
      return 90;
    case "vm":
      return 100;
    default:
      return 104;
  }
}

export const CLUSTER_COLORS: Record<string, string> = {
  docker: "#22c55e",
  podman: "#3b82f6",
  kubernetes: "#a78bfa",
  vm: "#f59e0b",
  apps: "#94a3b8",
  system: "#f87171",
};

const CLUSTER_LABELS: Record<string, string> = {
  docker: "Docker",
  podman: "Podman",
  kubernetes: "Kubernetes",
  vm: "VMs",
  apps: "Apps",
  system: "System (root)",
};

export const TopologyCanvas: Component<{
  nodes: TopologyNode[];
  edges: TopologyEdge[];
  nodeOrigin?: Map<string, string>;
  showGroups?: boolean;
  selectedId?: string | null;
  onSelect?: (id: string | null) => void;
  paused?: boolean;
}> = (props) => {
  let svgRef!: SVGSVGElement;
  const [zoom, setZoom] = createSignal(1);
  const [pan, setPan] = createSignal({ x: 0, y: 0 });
  const [positions, setPositions] = createSignal<Map<string, Pos>>(new Map());

  const posCache = new Map<string, Pos>();
  let drag: { sx: number; sy: number; tx: number; ty: number } | null = null;
  let didPan = false;
  let downTarget: Element | null = null;

  const idKey = createMemo(
    () =>
      untrack(() => props.nodes)
        .map((n) => n.id)
        .sort()
        .join(",") +
      "|" +
      untrack(() => props.edges)
        .map((e) => `${e.source}->${e.target}`)
        .sort()
        .join(","),
  );

  const clusterKeyOf = (node: TopologyNode): string | undefined => {
    const o = props.nodeOrigin?.get(node.id);
    if (o) return o;
    return undefined;
  };

  function elkLayout(nodes: TopologyNode[], edges: TopologyEdge[], then: () => void) {
    const graph = {
      id: "root",
      layoutOptions: {
        "elk.algorithm": "layered",
        "elk.direction": "RIGHT",
        "elk.layered.spacing.nodeNodeBetweenLayers": "90",
        "elk.spacing.nodeNode": "40",
      },
      children: nodes.map((n) => ({
        id: n.id,
        width: NODE_W,
        height: nodeHeight(n.kind),
      })),
      edges: edges.map((e) => ({
        id: `${e.id}`,
        sources: [e.source],
        targets: [e.target],
      })),
    };
    elk
      .layout(graph)
      .then((result) => {
        for (const child of result.children ?? []) {
          posCache.set(child.id, {
            x: child.x ?? 0,
            y: child.y ?? 0,
            w: child.width ?? NODE_W,
            h: child.height ?? nodeHeight("container"),
          });
        }
        then();
        setPositions(new Map(posCache));
        fit();
      })
      .catch(() => {
        posCache.clear();
        let i = 0;
        for (const n of nodes) {
          posCache.set(n.id, {
            x: 40 + (i % 5) * 270,
            y: 40 + Math.floor(i / 5) * 170,
            w: NODE_W,
            h: nodeHeight(n.kind),
          });
          i++;
        }
        then();
        setPositions(new Map(posCache));
        fit();
      });
  }

  function spawnNew(
    nodes: TopologyNode[],
    edges: TopologyEdge[],
    missingIds: Set<string>,
  ) {
    const nb = new Map<string, Set<string>>();
    for (const e of edges) {
      if (posCache.has(e.source) || posCache.has(e.target)) {
        if (!nb.has(e.source)) nb.set(e.source, new Set());
        if (!nb.has(e.target)) nb.set(e.target, new Set());
        nb.get(e.source)!.add(e.target);
        nb.get(e.target)!.add(e.source);
      }
    }
    const centroids = new Map<string, { x: number; y: number; n: number }>();
    for (const n of nodes) {
      const p = posCache.get(n.id);
      if (!p) continue;
      const k = clusterKeyOf(n) ?? "__";
      const c = centroids.get(k) ?? { x: 0, y: 0, n: 0 };
      c.x += p.x;
      c.y += p.y;
      c.n++;
      centroids.set(k, c);
    }
    for (const c of centroids.values()) {
      if (c.n) {
        c.x /= c.n;
        c.y /= c.n;
      }
    }
    const fallback = { x: 0, y: 0, n: 0 };
    let fx = 0, fy = 0, fn = 0;
    for (const p of posCache.values()) {
      fx += p.x;
      fy += p.y;
      fn++;
    }
    fallback.x = fn ? fx / fn : 200;
    fallback.y = fn ? fy / fn : 150;

    const place = (n: TopologyNode) => {
      const c = centroids.get(clusterKeyOf(n) ?? "__") ?? fallback;
      const tries: Array<{ x: number; y: number }> = [];
      for (const anchorId of nb.get(n.id) ?? []) {
        const a = posCache.get(anchorId);
        if (!a) continue;
        for (let t = 0; t < 8; t++) {
          const angle = Math.random() * Math.PI * 2;
          tries.push({
            x: a.x + Math.cos(angle) * (150 + Math.random() * 130),
            y: a.y + Math.sin(angle) * (100 + Math.random() * 110),
          });
        }
      }
      for (let t = 0; t < 14; t++) {
        tries.push({
          x: c.x + (Math.random() - 0.5) * 560,
          y: c.y + (Math.random() - 0.5) * 360,
        });
      }
      for (const t of tries) {
        if (!collides(t.x, t.y)) {
          posCache.set(n.id, { x: t.x, y: t.y, w: NODE_W, h: nodeHeight(n.kind) });
          return true;
        }
      }
      posCache.set(n.id, {
        x: c.x + Math.random() * 700,
        y: c.y + Math.random() * 460,
        w: NODE_W,
        h: nodeHeight(n.kind),
      });
      return true;
    };

    for (const n of nodes) {
      if (!missingIds.has(n.id)) continue;
      place(n);
    }
    setPositions(new Map(posCache));
  }

  function collides(x: number, y: number): boolean {
    for (const p of posCache.values()) {
      if (Math.abs(p.x - x) < NODE_W - 30 && Math.abs(p.y - y) < 90) return true;
    }
    return false;
  }

  function arrange() {
    const nodes = props.nodes;
    const edges = props.edges;
    if (nodes.length === 0) return;
    posCache.clear();
    elkLayout(nodes, edges, () => undefined);
  }

  function fit() {
    if (posCache.size === 0 || !svgRef) return;
    const parent = svgRef.getBoundingClientRect();
    let minX = Infinity,
      minY = Infinity,
      maxX = -Infinity,
      maxY = -Infinity;
    for (const p of posCache.values()) {
      minX = Math.min(minX, p.x - 50);
      minY = Math.min(minY, p.y - 50);
      maxX = Math.max(maxX, p.x + p.w + 50);
      maxY = Math.max(maxY, p.y + p.h + 50);
    }
    const bw = Math.max(1, maxX - minX);
    const bh = Math.max(1, maxY - minY);
    const z = Math.max(
      MIN_ZOOM,
      Math.min(MAX_ZOOM, Math.min(parent.width / bw, parent.height / bh)),
    );
    setZoom(z);
    setPan({
      x: (parent.width - bw * z) / 2 - minX * z,
      y: (parent.height - bh * z) / 2 - minY * z,
    });
  }

  function zoomAt(cx: number, cy: number, f: number) {
    if (!svgRef) return;
    const rect = svgRef.getBoundingClientRect();
    const mx = cx - rect.left;
    const my = cy - rect.top;
    const k = zoom();
    const nk = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, k * f));
    if (nk === k) return;
    setPan({
      x: mx - ((mx - pan().x) * nk) / k,
      y: my - ((my - pan().y) * nk) / k,
    });
    setZoom(nk);
  }

  const centerX = () => {
    if (!svgRef) return 0;
    const r = svgRef.getBoundingClientRect();
    return r.left + r.width / 2;
  };
  const centerY = () => {
    if (!svgRef) return 0;
    const r = svgRef.getBoundingClientRect();
    return r.top + r.height / 2;
  };

  createEffect(() => {
    idKey();
    untrack(() => {
      const nodes = props.nodes;
      const edges = props.edges;
      if (nodes.length === 0) {
        posCache.clear();
        setPositions(new Map());
        return;
      }
      const missingIds = new Set(
        nodes.map((n) => n.id).filter((id) => !posCache.has(id)),
      );
      if (missingIds.size === 0) return;
      if (posCache.size === 0) {
        elkLayout(
          nodes,
          edges,
          () => undefined,
        );
      } else {
        spawnNew(nodes, edges, missingIds);
      }
    });
  });

  onMount(() => {
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      zoomAt(e.clientX, e.clientY, Math.exp(-e.deltaY * 0.0015));
    };
    svgRef.addEventListener("wheel", onWheel, { passive: false });
    onCleanup(() => svgRef.removeEventListener("wheel", onWheel));
  });

  const startPan = (e: PointerEvent) => {
    if (e.button !== 0) return;
    drag = { sx: e.clientX, sy: e.clientY, tx: pan().x, ty: pan().y };
    downTarget = e.target as Element | null;
    didPan = false;
    svgRef.setPointerCapture(e.pointerId);
  };

  const movePan = (e: PointerEvent) => {
    if (!drag) return;
    const dx = e.clientX - drag.sx;
    const dy = e.clientY - drag.sy;
    if (!didPan && Math.abs(dx) + Math.abs(dy) > 4) didPan = true;
    if (didPan) setPan({ x: drag.tx + dx, y: drag.ty + dy });
  };

  const endPan = () => {
    if (drag && !didPan && props.onSelect) {
      const el = downTarget?.closest?.("[data-node-id]") as HTMLElement | null;
      const id = el?.getAttribute("data-node-id") ?? null;
      props.onSelect((id && id !== "") ? (props.selectedId === id ? null : id) : null);
    }
    downTarget = null;
    drag = null;
    setTimeout(() => (didPan = false), 0);
  };

  const layoutMap = createMemo(() => {
    const m = new Map<string, Pos>();
    for (const [id, p] of positions()) m.set(id, p);
    return m;
  });

  const centerOf = (id: string): { x: number; y: number; ok: boolean } => {
    const p = layoutMap().get(id);
    if (!p) return { x: 0, y: 0, ok: false };
    return { x: p.x + p.w / 2, y: p.y + p.h / 2, ok: true };
  };

  const clusters = createMemo(() => {
    if (!props.showGroups) return [];
    const groups = new Map<string, Pos[]>();
    for (const n of props.nodes) {
      const k = clusterKeyOf(n);
      if (!k) continue;
      const p = positions().get(n.id);
      if (!p) continue;
      const arr = groups.get(k) ?? groups.set(k, []).get(k)!;
      arr.push(p);
    }
    const out: Array<{
      key: string;
      label: string;
      color: string;
      x: number;
      y: number;
      w: number;
      h: number;
    }> = [];
    for (const [key, ps] of groups) {
      let minX = Infinity,
        minY = Infinity,
        maxX = -Infinity,
        maxY = -Infinity;
      for (const p of ps) {
        minX = Math.min(minX, p.x);
        minY = Math.min(minY, p.y);
        maxX = Math.max(maxX, p.x + p.w);
        maxY = Math.max(maxY, p.y + p.h);
      }
      out.push({
        key,
        label: CLUSTER_LABELS[key] ?? key,
        color: CLUSTER_COLORS[key] ?? "#94a3b8",
        x: minX - CLUSTER_PAD,
        y: minY - CLUSTER_PAD - 14,
        w: maxX - minX + CLUSTER_PAD * 2,
        h: maxY - minY + CLUSTER_PAD * 2 + 14,
      });
    }
    return out;
  });

  return (
    <div class="relative flex-1 min-h-0 overflow-hidden bg-surface-950">
      <svg
        ref={svgRef}
        class="absolute inset-0 h-full w-full touch-none select-none"
        onPointerDown={startPan}
        onPointerMove={movePan}
        onPointerUp={endPan}
        onPointerCancel={endPan}
      >
        <g transform={`translate(${pan().x},${pan().y}) scale(${zoom()})`}>
          <defs>
            <pattern id="tgrid" width="24" height="24" patternUnits="userSpaceOnUse">
              <path
                d="M 24 0 L 0 0 0 24"
                fill="none"
                class="stroke-surface-800"
                stroke-width="0.5"
              />
            </pattern>
          </defs>
          <rect x="-10000" y="-10000" width="30000" height="30000" fill="url(#tgrid)" />
          <For each={clusters()}>
            {(c) => (
              <g>
                <rect
                  x={c.x}
                  y={c.y}
                  width={c.w}
                  height={c.h}
                  rx="14"
                  fill="none"
                  stroke={c.color}
                  stroke-opacity="0.35"
                  stroke-width="1.5"
                  stroke-dasharray="6 5"
                />
                <text
                  x={c.x + 12}
                  y={c.y + 16}
                  fill={c.color}
                  fill-opacity="0.85"
                  font-size="11"
                  font-weight="600"
                >
                  {c.label}
                </text>
              </g>
            )}
          </For>
          <For each={props.edges}>
            {(edge) => {
              const s = centerOf(edge.source);
              const t = centerOf(edge.target);
              if (!s.ok || !t.ok) return null;
              return (
                <Edge
                  edge={edge}
                  paused={props.paused}
                  points={[
                    { x: s.x, y: s.y },
                    { x: t.x, y: t.y },
                  ]}
                />
              );
            }}
          </For>
          <For each={props.nodes}>
            {(node) => {
              const p = layoutMap().get(node.id);
              if (!p) return null;
              return (
                <g
                  data-node-id={node.id}
                  class={props.onSelect ? "cursor-pointer" : undefined}
                >
                  <Show
                    when={props.selectedId === node.id}
                    fallback={null}
                  >
                    <rect
                      x={p.x - 5}
                      y={p.y - 5}
                      width={p.w + 10}
                      height={p.h + 10}
                      rx="11"
                      fill="none"
                      stroke="#22c55e"
                      stroke-opacity="0.8"
                      stroke-width="2"
                    />
                  </Show>
                  <Node
                    node={node}
                    origin={props.nodeOrigin?.get(node.id)}
                    selected={props.selectedId === node.id}
                    x={p.x}
                    y={p.y}
                    width={p.w}
                    height={p.h}
                  />
                </g>
              );
            }}
          </For>
        </g>
      </svg>

      <div class="absolute bottom-3 right-3 flex items-center gap-0.5 rounded-md border border-surface-800 bg-surface-900/90 px-1 py-1 shadow">
        <CtrlBtn label="−" onClick={() => zoomAt(centerX(), centerY(), 1 / 1.25)} />
        <span class="w-10 text-center font-mono text-[10px] text-ink-100">
          {Math.round(zoom() * 100)}%
        </span>
        <CtrlBtn label="＋" onClick={() => zoomAt(centerX(), centerY(), 1.25)} />
        <CtrlBtn label="Fit" onClick={fit} />
        <CtrlBtn label="Arrange" onClick={arrange} />
      </div>
      <div class="absolute top-3 right-3 font-mono text-[10px] text-ink-700">
        {props.nodes.length} nodes · {props.edges.length} edges
      </div>
    </div>
  );
};

const CtrlBtn: Component<{ label: string; onClick: () => void }> = (props) => (
  <button
    onClick={props.onClick}
    class="px-1.5 py-0.5 rounded text-[11px] text-ink-700 hover:text-ink-100 transition-colors"
  >
    {props.label}
  </button>
);
