import { createEffect, onCleanup, onMount } from "solid-js";
import uPlot from "uplot";
import { themeStore } from "@/stores/themeStore";

interface SparklineProps {
  data: number[];
  color?: string;
  height?: number;
}

function toData(d: number[]): uPlot.AlignedData {
  return [d.map((_, i) => i), d];
}

function chartPalette() {
  const cs = getComputedStyle(document.documentElement);
  const v = (name: string) => `rgb(${cs.getPropertyValue(name).trim()})`;
  return { axis: v("--ink-700"), grid: v("--chart-grid") };
}

export function Sparkline(props: SparklineProps) {
  let el!: HTMLDivElement;
  let chart: uPlot | undefined;
  const color = () => props.color ?? "#22c55e";
  const h = () => props.height ?? 56;

  const build = () => {
    if (chart) {
      chart.destroy();
      chart = undefined;
    }
    const pal = chartPalette();
    const opts: uPlot.Options = {
      width: Math.max(el.clientWidth, 60),
      height: h(),
      cursor: { show: false },
      legend: { show: false },
      scales: { x: { time: false } },
      axes: [{ show: false }, { show: false }],
      series: [
        {},
        {
          stroke: color(),
          width: 1.5,
          fill: `${color()}22`,
          points: { show: false },
        },
      ],
    };
    chart = new uPlot(opts, toData(props.data), el);
  };

  onMount(() => {
    build();
    const ro = new ResizeObserver(() => {
      if (el.clientWidth > 0) {
        chart?.setSize({ width: Math.max(el.clientWidth, 60), height: h() });
      }
    });
    ro.observe(el);
    onCleanup(() => {
      ro.disconnect();
      chart?.destroy();
    });
  });

  createEffect(() => {
    themeStore.theme();
    if (el) build();
  });

  createEffect(() => {
    const d = toData(props.data);
    if (chart) {
      chart.setData(d);
    }
  });

  return <div ref={el} class="w-full" style={{ height: `${h()}px` }} />;
}
