import { Component, For, Show, createMemo } from "solid-js";
import { useParams, A } from "@solidjs/router";
import { runtimeStore } from "@/stores/runtimeStore";
import type { RuntimeKind } from "@/types";
import { fmtBps, fmtBytes, fmtPct, stateColor } from "@/lib/format";
import { Sparkline } from "@/components/charts/Sparkline";
import { LineChart } from "@/components/charts/LineChart";
import { NotFound } from "@/views/NotFound";

const VALID: RuntimeKind[] = ["docker", "podman"];

const GREEN = "#22c55e";
const BLUE = "#3b82f6";

export const RuntimeDashboard: Component = () => {
  const params = useParams<{ runtime: string }>();
  const kind = params.runtime as RuntimeKind;

  return (
    <Show when={VALID.includes(kind)} fallback={<NotFound label={params.runtime} />}>
      <RuntimeDashboardInner kind={kind} />
    </Show>
  );
};

const RuntimeDashboardInner: Component<{ kind: RuntimeKind }> = (props) => {
  const snap = () => runtimeStore.snap(props.kind);
  const running = () => snap().containers.filter((c) => c.state === "running");
  const stopped = () => snap().containers.filter((c) => c.state !== "running");
  const last = (arr?: number[]) => (arr && arr.length > 0 ? arr[arr.length - 1] : 0);
  const topContainers = createMemo(() =>
    [...running()].sort((a, b) => b.cpu_percent - a.cpu_percent).slice(0, 6),
  );

  return (
    <div class="p-5 space-y-5">
      <div>
        <h2 class="text-lg font-semibold capitalize">{props.kind}</h2>
        <p class="text-xs text-ink-700 truncate">{snap().detail ?? "—"}</p>
      </div>

      <div class="grid grid-cols-4 gap-4">
        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <span class="text-xs text-ink-700 uppercase tracking-wider">Containers</span>
          <div class="mt-2 flex items-baseline gap-2">
            <span class="text-3xl font-semibold text-ok">{running().length}</span>
            <span class="text-xs text-ink-700">running</span>
            <span class="text-2xl font-semibold text-ink-700">{stopped().length}</span>
            <span class="text-xs text-ink-700">stopped</span>
          </div>
        </div>

        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <span class="text-xs text-ink-700 uppercase tracking-wider">Aggregate CPU</span>
          <div class="text-3xl font-semibold mt-1">{fmtPct(last(snap().history.cpu), 0)}</div>
          <Sparkline data={snap().history.cpu} color={GREEN} height={44} />
        </div>

        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <span class="text-xs text-ink-700 uppercase tracking-wider">Aggregate Memory</span>
          <div class="text-3xl font-semibold mt-1">{fmtBytes(last(snap().history.mem_bytes), 0)}</div>
          <Sparkline
            data={snap().history.mem_bytes.map((b) => b / (1024 * 1024))}
            color={BLUE}
            height={44}
          />
        </div>

        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <span class="text-xs text-ink-700 uppercase tracking-wider">Network</span>
          <div class="text-sm font-mono mt-2 space-y-0.5">
            <div class="flex justify-between">
              <span class="text-ok">↓ RX</span>
              <span>{fmtBps(last(snap().history.net_rx_bps))}</span>
            </div>
            <div class="flex justify-between">
              <span class="text-info">↑ TX</span>
              <span>{fmtBps(last(snap().history.net_tx_bps))}</span>
            </div>
          </div>
          <Sparkline data={snap().history.net_rx_bps} color={GREEN} height={28} />
        </div>
      </div>

      <div class="grid grid-cols-2 gap-4">
        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <h3 class="text-xs text-ink-700 uppercase tracking-wider mb-3">
            Aggregate CPU % (60 samples)
          </h3>
          <LineChart
            series={[{ label: "CPU %", color: GREEN, data: snap().history.cpu }]}
            height={180}
          />
        </div>
        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <h3 class="text-xs text-ink-700 uppercase tracking-wider mb-3">Memory (MB)</h3>
          <LineChart
            series={[
              {
                label: "MEM",
                color: BLUE,
                data: snap().history.mem_bytes.map((b) => b / (1024 * 1024)),
              },
            ]}
            height={180}
          />
        </div>
      </div>

      <div class="bg-surface-900 border border-surface-800 rounded-lg">
        <div class="flex items-center justify-between px-4 py-2.5 border-b border-surface-800">
          <h3 class="text-xs text-ink-700 uppercase tracking-wider">
            Busiest containers
          </h3>
          <A
            href={`/${props.kind}/containers`}
            class="text-xs text-ok hover:text-ok"
          >
            View all →
          </A>
        </div>
        <For each={topContainers()}>
          {(c) => (
            <div class="flex items-center gap-4 px-4 py-2 border-b border-surface-800/50 last:border-0 text-sm">
              <span
                class="w-2 h-2 rounded-full shrink-0"
                style={{ "background-color": stateColor(c.state) }}
              />
              <A
                href={`/${props.kind}/containers/${c.id}`}
                class="font-medium hover:text-ok w-44 truncate"
              >
                {c.name}
              </A>
              <span class="text-xs text-ink-700 flex-1 truncate">{c.image}</span>
              <span class="text-xs font-mono w-16 text-right">{fmtPct(c.cpu_percent)}</span>
              <span class="text-xs font-mono w-24 text-right">{fmtBytes(c.memory_bytes)}</span>
            </div>
          )}
        </For>
        {topContainers().length === 0 && (
          <div class="px-4 py-8 text-center text-sm text-ink-700">
            No running containers
          </div>
        )}
      </div>
    </div>
  );
};
