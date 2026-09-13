import { Component, createMemo } from "solid-js";
import type { TopologyEdge } from "@/types";

interface EdgeProps {
  edge: TopologyEdge;
  points: Array<{ x: number; y: number }>;
  paused?: boolean;
}

export const Edge: Component<EdgeProps> = (props) => {
  const pathD = createMemo(() => {
    if (props.points.length < 2) return "";
    const pts = props.points;
    let d = `M ${pts[0].x} ${pts[0].y}`;
    for (let i = 1; i < pts.length; i++) {
      d += ` L ${pts[i].x} ${pts[i].y}`;
    }
    return d;
  });

  const animDuration = createMemo(() => {
    const bps = Math.max(props.edge.traffic_bps, 1);
    return `${Math.max(0.2, 10 / (bps / 1000))}s`;
  });

  const hasTraffic = () => props.edge.traffic_bps > 0;

  const mid = createMemo(() => {
    if (props.points.length < 2) return null;
    const a = props.points[0];
    const b = props.points[props.points.length - 1];
    return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 - 6 };
  });

  return (
    <g>
      <path
        d={pathD()}
        fill="none"
        class="stroke-edge"
        stroke-width="1.5"
        stroke-opacity="0.6"
      />
      {hasTraffic() && (
        <path
          d={pathD()}
          fill="none"
          stroke="#22c55e"
          stroke-width="2"
          stroke-dasharray="8 4"
          stroke-linecap="round"
          style={{
            animation: `dash ${animDuration()} linear infinite`,
            "animation-play-state": props.paused ? "paused" : "running",
          }}
        />
      )}
      {props.edge.label && mid() && (
        <text
          x={mid()!.x}
          y={mid()!.y}
          text-anchor="middle"
          class="fill-ink-700"
          font-size="9"
          font-family="monospace"
        >
          {props.edge.label}
        </text>
      )}
    </g>
  );
};
