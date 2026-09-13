import { createEffect, onCleanup, onMount } from "solid-js";
import uPlot from "uplot";
import { themeStore } from "@/stores/themeStore";

export interface ChartSeries {
  label: string;
  color: string;
  data: number[];
}

interface LineChartProps {
  series: ChartSeries[];
  height?: number;
}

function toData(series: ChartSeries[]): uPlot.AlignedData {
  const len = Math.max(0, ...series.map((s) => s.data.length));
  const x = Array.from({ length: len }, (_, i) => i);
  const cols = series.map((s) => {
    const arr: (number | null)[] = new Array(len).fill(null);
    const offset = len - s.data.length;
    s.data.forEach((v, i) => {
      arr[i + offset] = v;
    });
    return arr;
  });
  return [x, ...cols];
}

function chartPalette() {
  const cs = getComputedStyle(document.documentElement);
  const v = (name: string) => `rgb(${cs.getPropertyValue(name).trim()})`;
  return { axis: v("--ink-700"), grid: v("--chart-grid") };
}

export function LineChart(props: LineChartProps) {
  let el!: HTMLDivElement;
  let chart: uPlot | undefined;
  const h = () => props.height ?? 220;

  const build = () => {
    if (chart) {
      chart.destroy();
      chart = undefined;
    }
    const pal = chartPalette();
    const opts: uPlot.Options = {
      width: Math.max(el.clientWidth, 60),
      height: h(),
      scales: { x: { time: false } },
      legend: { show: true, live: true, markers: { width: 6 } },
      axes: [
        {
          stroke: pal.axis,
          grid: { show: true, stroke: pal.grid, width: 1 },
          ticks: { stroke: pal.grid },
        },
        {
          stroke: pal.axis,
          grid: { show: true, stroke: pal.grid, width: 1 },
          ticks: { stroke: pal.grid },
        },
      ],
      series: [
        {},
        ...props.series.map((s) => ({
          label: s.label,
          stroke: s.color,
          width: 2,
          points: { show: false },
        })),
      ],
    };
    chart = new uPlot(opts, toData(props.series), el);
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
    const d = toData(props.series);
    if (chart) {
      chart.setData(d);
    }
  });

  return <div ref={el} class="w-full" style={{ height: `${h()}px` }} />;
}
