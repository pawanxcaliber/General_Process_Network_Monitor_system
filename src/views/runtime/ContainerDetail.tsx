import { Component, For, Show, createEffect, createSignal } from "solid-js";
import { useParams, A } from "@solidjs/router";
import { runtimeStore } from "@/stores/runtimeStore";
import { logsStore } from "@/stores/logsStore";
import { useLogStream } from "@/hooks/useLogStream";
import type { RuntimeKind } from "@/types";
import { fmtBps, fmtBytes, fmtPct, timeAgo, stateColor } from "@/lib/format";
import { LineChart } from "@/components/charts/LineChart";

const GREEN = "#22c55e";
const BLUE = "#3b82f6";

export const ContainerDetail: Component = () => {
  const params = useParams<{ runtime: string; id: string }>();
  const kind = params.runtime as RuntimeKind;
  const id = params.id;
  const c = () => runtimeStore.snap(kind).containers.find((x) => x.id === id);
  const { streaming } = useLogStream(kind, id);

  const memSeries = () => (c()?.history.mem_bytes ?? []).map((b) => b / (1024 * 1024));
  const memLimitMB = () => (c()?.memory_limit_bytes ?? 0) / (1024 * 1024);

  return (
    <Show
      when={c()}
      fallback={<div class="p-10 text-center text-sm text-ink-700">Loading container…</div>}
    >
      {(cc) => (
        <div class="p-5 space-y-4">
          <div class="flex items-start justify-between">
            <div class="flex items-center gap-3">
              <A href={`/${kind}/containers`} class="text-ink-700 hover:text-ink-100 text-sm">
                ←
              </A>
              <span
                class="w-2.5 h-2.5 rounded-full mt-0.5"
                style={{ "background-color": stateColor(cc().state) }}
              />
              <div>
                <h2 class="text-lg font-semibold">{cc().name}</h2>
                <p class="text-xs text-ink-700 font-mono">{cc().image}</p>
              </div>
            </div>
            <div class="text-right text-xs space-y-1">
              <div
                class="inline-block px-2 py-0.5 rounded-full border capitalize"
                style={{
                  color: stateColor(cc().state),
                  "border-color": `${stateColor(cc().state)}44`,
                  "background-color": `${stateColor(cc().state)}11`,
                }}
              >
                {cc().state}
              </div>
              <div class="text-ink-700">{cc().status}</div>
              <div class="text-ink-800 font-mono">
                created {cc().created > 0 ? timeAgo(cc().created) : "—"}
              </div>
            </div>
          </div>

          <div class="grid grid-cols-4 gap-3 text-xs">
            <div class="bg-surface-900 border border-surface-800 rounded-lg p-3">
              <span class="text-ink-700 uppercase tracking-wider text-[10px]">CPU</span>
              <div class="text-xl font-semibold mt-1">
                {cc().state === "running" ? fmtPct(cc().cpu_percent) : "—"}
              </div>
            </div>
            <div class="bg-surface-900 border border-surface-800 rounded-lg p-3">
              <span class="text-ink-700 uppercase tracking-wider text-[10px]">Memory</span>
              <div class="text-xl font-semibold mt-1">
                {cc().state === "running" ? fmtBytes(cc().memory_bytes) : "—"}
              </div>
              <div class="text-[10px] text-ink-700">
                of {fmtBytes(cc().memory_limit_bytes, 0)} ({fmtPct(cc().memory_percent)})
              </div>
            </div>
            <div class="bg-surface-900 border border-surface-800 rounded-lg p-3">
              <span class="text-ink-700 uppercase tracking-wider text-[10px]">Net I/O</span>
              <div class="text-sm font-mono mt-2">
                <div class="text-ok">↓ {fmtBps(cc().rx_bps)}</div>
                <div class="text-info">↑ {fmtBps(cc().tx_bps)}</div>
              </div>
              <div class="text-[10px] text-ink-700 mt-1">
                total ↓{fmtBytes(cc().rx_total, 0)} ↑{fmtBytes(cc().tx_total, 0)}
              </div>
            </div>
            <div class="bg-surface-900 border border-surface-800 rounded-lg p-3">
              <span class="text-ink-700 uppercase tracking-wider text-[10px]">Networks</span>
              <div class="mt-2 flex flex-wrap gap-1">
                <For each={cc().networks}>
                  {(n) => (
                    <span class="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/10 text-info border border-blue-500/20 font-mono">
                      {n}
                    </span>
                  )}
                </For>
                {cc().networks.length === 0 && <span class="text-ink-700">none</span>}
              </div>
            </div>
          </div>

          <div class="grid grid-cols-2 gap-4">
            <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
              <h3 class="text-xs text-ink-700 uppercase tracking-wider mb-3">
                CPU % (last 60 samples)
              </h3>
              <LineChart
                series={[{ label: "CPU %", color: GREEN, data: cc().history.cpu ?? [] }]}
                height={170}
              />
            </div>
            <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
              <h3 class="text-xs text-ink-700 uppercase tracking-wider mb-3">
                Memory (MB{memLimitMB() > 0 ? `, limit ${memLimitMB().toFixed(0)}` : ""})
              </h3>
              <LineChart series={[{ label: "MEM", color: BLUE, data: memSeries() }]} height={170} />
            </div>
          </div>

          <Show when={cc().ports.length > 0}>
            <div class="bg-surface-900 border border-surface-800 rounded-lg overflow-hidden">
              <h3 class="text-xs text-ink-700 uppercase tracking-wider px-4 py-2.5 border-b border-surface-800">
                Port mappings
              </h3>
              <table class="w-full text-xs">
                <thead>
                  <tr class="text-ink-700 border-b border-surface-800">
                    <th class="text-left px-4 py-2 font-medium">Host port</th>
                    <th class="text-left px-2 py-2 font-medium">Container port</th>
                    <th class="text-left px-2 py-2 font-medium">Protocol</th>
                  </tr>
                </thead>
                <tbody class="font-mono">
                  <For each={cc().ports}>
                    {(p) => (
                      <tr class="border-b border-surface-800/50 last:border-0">
                        <td class="px-4 py-1.5">
                          {p.host_port ? (
                            <span class="text-ok">{p.host_port}</span>
                          ) : (
                            <span class="text-ink-700">—</span>
                          )}
                        </td>
                        <td class="px-2 py-1.5">{p.container_port}</td>
                        <td class="px-2 py-1.5 uppercase text-ink-700">{p.proto}</td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </div>
          </Show>

          <LogViewer logKey={`container-logs:${kind}:${id}`} streaming={streaming()} />
        </div>
      )}
    </Show>
  );
};

export const LogViewer: Component<{ logKey: string; streaming: boolean }> = (props) => {
  let scrollRef: HTMLDivElement | undefined;
  const [autoScroll, setAutoScroll] = createSignal(true);
  const lines = () => logsStore.lines(props.logKey);

  createEffect(() => {
    lines().length;
    if (autoScroll() && scrollRef) {
      scrollRef.scrollTop = scrollRef.scrollHeight;
    }
  });

  return (
    <div class="bg-surface-900 border border-surface-800 rounded-lg overflow-hidden">
      <div class="flex items-center justify-between px-4 py-2 border-b border-surface-800">
        <div class="flex items-center gap-2">
          <h3 class="text-xs text-ink-700 uppercase tracking-wider">Logs</h3>
          <span class="flex items-center gap-1 text-[10px]">
            <span
              class={`w-1.5 h-1.5 rounded-full ${
                props.streaming ? "bg-green-400 animate-pulse" : "bg-surface-700"
              }`}
            />
            {props.streaming ? "streaming" : "idle"}
          </span>
        </div>
        <div class="flex items-center gap-2">
          <span class="text-[10px] text-ink-700 font-mono">{lines().length} lines</span>
          <button
            onClick={() => setAutoScroll((v) => !v)}
            class={`text-[11px] px-2 py-0.5 rounded border transition-colors ${
              autoScroll()
                ? "border-green-500/30 text-ok bg-green-500/10"
                : "border-surface-800 text-ink-700 hover:text-ink-100"
            }`}
          >
            {autoScroll() ? "Following" : "Paused"}
          </button>
          <button
            onClick={() => logsStore.clear(props.logKey)}
            class="text-[11px] px-2 py-0.5 rounded border border-surface-800 text-ink-700 hover:text-ink-100 transition-colors"
          >
            Clear
          </button>
        </div>
      </div>
      <div
        ref={scrollRef}
        class="h-72 overflow-y-auto bg-surface-950/60 font-mono text-[11px] leading-5 p-3"
      >
        <For each={lines()}>
          {(line) => (
            <div class="flex gap-2 whitespace-pre-wrap break-all">
              <Show when={line.ts}>
                <span class="text-ink-800 shrink-0 select-none">{line.ts}</span>
              </Show>
              <span
                class={
                  line.stream === "stderr"
                    ? "text-bad"
                    : line.stream === "status"
                      ? "text-warn italic"
                      : "text-ink-100"
                }
              >
                {line.text}
              </span>
            </div>
          )}
        </For>
        {lines().length === 0 && (
          <div class="text-center text-ink-700 py-16">
            Waiting for logs… (exited containers show their historical output)
          </div>
        )}
      </div>
    </div>
  );
};
