import { Component, For, Show, createSignal } from "solid-js";
import { appInvoke as invoke } from "@/lib/transport";
import { hostStore } from "@/stores/hostStore";
import { fmtBps, fmtBytes, fmtMem } from "@/lib/format";
import { portOwner } from "@/lib/ports";
import type { ProcessSortKey } from "@/types";

const SORT_OPTIONS: Array<{ key: ProcessSortKey; label: string }> = [
  { key: "cpu", label: "CPU" },
  { key: "mem", label: "MEM" },
  { key: "disk", label: "DISK" },
  { key: "net", label: "NET" },
  { key: "gpu", label: "GPU" },
  { key: "igpu", label: "iGPU" },
  { key: "dgpu", label: "dGPU" },
  { key: "pid", label: "PID" },
  { key: "name", label: "Name" },
];

export const Host: Component = () => {
  const host = () => hostStore.host();
  const ports = () => host()?.ports ?? [];
  const tcpCount = () => ports().filter((p) => p.proto === "tcp").length;
  const udpCount = () => ports().filter((p) => p.proto === "udp").length;
  const [sortKey, setSortKey] = createSignal<ProcessSortKey>("cpu");

  const changeSort = (key: ProcessSortKey) => {
    setSortKey(key);
    invoke("set_process_sort", { key }).catch(() => {});
  };

  return (
    <div class="p-5 space-y-5">
      <div>
        <h2 class="text-lg font-semibold">Host</h2>
        <p class="text-xs text-ink-700">System-level resource breakdown</p>
      </div>

      <section class="bg-surface-900 border border-surface-800 rounded-lg p-4">
        <div class="flex items-baseline justify-between mb-3">
          <h3 class="text-xs text-ink-700 uppercase tracking-wider">
            CPU cores ({host()?.cpu_per_core.length ?? 0})
          </h3>
          <span class="text-sm font-mono">{(host()?.cpu_percent ?? 0).toFixed(1)}% overall</span>
        </div>
        <div class="max-h-[152px] overflow-y-auto pr-1">
          <div class="grid grid-cols-2 gap-x-6 gap-y-2">
            <For each={host()?.cpu_per_core ?? []}>
              {(usage, i) => (
                <div class="flex items-center gap-2">
                  <span class="text-[10px] font-mono text-ink-700 w-8 shrink-0">
                    core{i()}
                  </span>
                  <div class="h-2 flex-1 bg-surface-800 rounded-full overflow-hidden">
                    <div
                      class="h-full rounded-full transition-all duration-500"
                      style={{
                        width: `${Math.min(usage, 100)}%`,
                        "background-color":
                          usage > 80 ? "rgb(var(--bad))" : usage > 50 ? "rgb(var(--warn))" : "rgb(var(--ok))",
                      }}
                    />
                  </div>
                  <span class="text-[10px] font-mono w-10 text-right text-ink-100">
                    {usage.toFixed(0)}%
                  </span>
                </div>
              )}
            </For>
          </div>
        </div>
      </section>

      <section class="bg-surface-900 border border-surface-800 rounded-lg p-4">
        <h3 class="text-xs text-ink-700 uppercase tracking-wider mb-3">Memory</h3>
        <MemBar
          label="RAM"
          used={host()?.ram_used_bytes ?? 0}
          total={host()?.ram_total_bytes ?? 0}
        />
        <ShowSwap used={host()?.swap_used_bytes ?? 0} total={host()?.swap_total_bytes ?? 0} />
      </section>

      <section class="bg-surface-900 border border-surface-800 rounded-lg overflow-hidden">
        <h3 class="text-xs text-ink-700 uppercase tracking-wider px-4 py-2.5 border-b border-surface-800">
          Disks ({host()?.disks.length ?? 0})
        </h3>
        <div class="max-h-[400px] overflow-y-auto">
          <table class="w-full text-sm">
            <thead class="sticky top-0 z-10">
              <tr class="text-[10px] uppercase tracking-wider text-ink-700 bg-surface-900 border-b border-surface-800">
                <th class="text-left px-4 py-2 font-medium">Mount</th>
                <th class="text-right px-2 py-2 font-medium">Read</th>
                <th class="text-right px-2 py-2 font-medium">Write</th>
                <th class="text-left px-4 py-2 font-medium w-64">Usage</th>
              </tr>
            </thead>
            <tbody>
              <For each={host()?.disks ?? []}>
                {(d) => {
                  const pct = d.total_bytes > 0 ? (d.used_bytes / d.total_bytes) * 100 : 0;
                  return (
                    <tr class="border-b border-surface-800/50 last:border-0">
                      <td class="px-4 py-2 font-mono text-xs">{d.name}</td>
                      <td class="px-2 py-2 text-right font-mono text-xs text-grape">
                        {fmtBps(d.read_bps)}
                      </td>
                      <td class="px-2 py-2 text-right font-mono text-xs text-warn">
                        {fmtBps(d.write_bps)}
                      </td>
                      <td class="px-4 py-2">
                        <div class="flex items-center gap-2">
                          <div class="h-1.5 flex-1 bg-surface-800 rounded-full overflow-hidden">
                            <div
                              class="h-full rounded-full"
                              style={{
                                width: `${Math.min(pct, 100)}%`,
                                "background-color":
                                  pct > 90 ? "rgb(var(--bad))" : pct > 70 ? "rgb(var(--warn))" : "rgb(var(--ok))",
                              }}
                            />
                          </div>
                          <span class="text-[10px] font-mono text-ink-700 w-24 text-right">
                            {fmtBytes(d.used_bytes, 0)} / {fmtBytes(d.total_bytes, 0)}
                          </span>
                        </div>
                      </td>
                    </tr>
                  );
                }}
              </For>
            </tbody>
          </table>
          {(host()?.disks.length ?? 0) === 0 && (
            <div class="px-4 py-10 text-center text-sm text-ink-700">No disks detected</div>
          )}
        </div>
      </section>

      <section class="bg-surface-900 border border-surface-800 rounded-lg overflow-hidden">
        <h3 class="text-xs text-ink-700 uppercase tracking-wider px-4 py-2.5 border-b border-surface-800">
          Network interfaces ({host()?.interfaces.length ?? 0})
        </h3>
        <div class="max-h-[400px] overflow-y-auto">
          <table class="w-full text-sm">
            <thead class="sticky top-0 z-10">
              <tr class="text-[10px] uppercase tracking-wider text-ink-700 bg-surface-900 border-b border-surface-800">
                <th class="text-left px-4 py-2 font-medium">Interface</th>
                <th class="text-right px-2 py-2 font-medium">RX rate</th>
                <th class="text-right px-2 py-2 font-medium">TX rate</th>
                <th class="text-right px-4 py-2 font-medium">RX total</th>
                <th class="text-right px-4 py-2 font-medium">TX total</th>
              </tr>
            </thead>
            <tbody>
              <For each={host()?.interfaces ?? []}>
                {(iface) => (
                  <tr class="border-b border-surface-800/50 last:border-0">
                    <td class="px-4 py-2 font-mono text-xs">{iface.name}</td>
                    <td class="px-2 py-2 text-right font-mono text-xs text-ok">
                      {fmtBps(iface.rx_bps)}
                    </td>
                    <td class="px-2 py-2 text-right font-mono text-xs text-info">
                      {fmtBps(iface.tx_bps)}
                    </td>
                    <td class="px-4 py-2 text-right font-mono text-xs text-ink-700">
                      {fmtBytes(iface.rx_total, 0)}
                    </td>
                    <td class="px-4 py-2 text-right font-mono text-xs text-ink-700">
                      {fmtBytes(iface.tx_total, 0)}
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
          {(host()?.interfaces.length ?? 0) === 0 && (
            <div class="px-4 py-10 text-center text-sm text-ink-700">
              No interfaces detected
            </div>
          )}
        </div>
      </section>

      <section class="bg-surface-900 border border-surface-800 rounded-lg overflow-hidden">
        <div class="flex items-center justify-between px-4 py-2.5 border-b border-surface-800">
          <h3 class="text-xs text-ink-700 uppercase tracking-wider">
            Processes ({host()?.processes.length ?? 0})
          </h3>
          <div class="relative">
            <select
              class="appearance-none text-[10px] font-mono text-ink-100 bg-surface-800 border border-surface-700 rounded pl-2 pr-6 py-0.5 outline-none cursor-pointer"
              value={sortKey()}
              onChange={(e) => changeSort(e.currentTarget.value as ProcessSortKey)}
            >
              <For each={SORT_OPTIONS}>
                {(o) => <option value={o.key}>sorted by {o.label}</option>}
              </For>
            </select>
            <svg
              class="pointer-events-none absolute right-1.5 top-1/2 -translate-y-1/2 w-3 h-3 text-ink-700"
              viewBox="0 0 12 12"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
            >
              <path d="M2.5 4.5 6 8l3.5-3.5" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </div>
        </div>
        <div class="max-h-[363px] overflow-y-auto">
          <table class="w-full text-sm">
            <thead class="sticky top-0 z-10">
              <tr class="text-[10px] uppercase tracking-wider text-ink-700 bg-surface-900 border-b border-surface-800">
                <th class="text-right px-4 py-2 font-medium w-16">PID</th>
                <th class="text-left px-2 py-2 font-medium">Name</th>
                <th class="text-right px-2 py-2 font-medium">CPU</th>
                <th class="text-right px-2 py-2 font-medium">MEM</th>
                <th class="text-right px-2 py-2 font-medium">DISK</th>
                <th class="text-right px-2 py-2 font-medium">NET</th>
                <th class="text-right px-2 py-2 font-medium">iGPU</th>
                <th class="text-right px-4 py-2 font-medium">dGPU</th>
              </tr>
            </thead>
            <tbody class="font-mono text-xs">
              <For each={host()?.processes ?? []}>
                {(p) => (
                  <tr class="border-b border-surface-800/50 last:border-0 hover:bg-surface-800/40">
                    <td class="px-4 py-2 text-right text-ink-700">{p.pid}</td>
                    <td
                      class="px-2 py-2 text-ink-100 max-w-[180px] truncate"
                      title={p.name}
                    >
                      {p.name}
                    </td>
                    <td
                      class={`px-2 py-2 text-right ${
                        p.cpu_percent > 80
                          ? "text-bad"
                          : p.cpu_percent > 50
                            ? "text-warn"
                            : "text-ok"
                      }`}
                    >
                      {p.cpu_percent.toFixed(1)}%
                    </td>
                    <td class="px-2 py-2 text-right text-ink-100">
                      {fmtBytes(p.memory_bytes, 0)}
                    </td>
                    <td class="px-2 py-2 text-right whitespace-nowrap">
                      <span class="text-grape">↓{fmtBps(p.disk_read_bps)}</span>{" "}
                      <span class="text-warn">↑{fmtBps(p.disk_write_bps)}</span>
                    </td>
                    <td class="px-2 py-2 text-right whitespace-nowrap">
                      <span class="text-ok">↓{fmtBps(p.net_rx_bps)}</span>{" "}
                      <span class="text-info">↑{fmtBps(p.net_tx_bps)}</span>
                    </td>
                    <td class="px-2 py-2 text-right text-ink-100">
                      {p.gpu_igpu_bytes ? fmtMem(p.gpu_igpu_bytes) : "—"}
                    </td>
                    <td class="px-4 py-2 text-right text-ink-100">
                      {p.gpu_dgpu_bytes ? fmtMem(p.gpu_dgpu_bytes) : "—"}
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
          {(host()?.processes.length ?? 0) === 0 && (
            <div class="px-4 py-10 text-center text-sm text-ink-700">No processes detected</div>
          )}
        </div>
      </section>

      <section class="bg-surface-900 border border-surface-800 rounded-lg overflow-hidden">
        <div class="flex items-center justify-between px-4 py-2.5 border-b border-surface-800">
          <h3 class="text-xs text-ink-700 uppercase tracking-wider">
            Active ports ({ports().length})
          </h3>
          <span class="text-[10px] font-mono text-ink-700">
            TCP {tcpCount()} · UDP {udpCount()}
          </span>
        </div>
        <div class="max-h-[400px] overflow-y-auto">
          <table class="w-full text-sm">
            <thead class="sticky top-0 z-10">
              <tr class="text-[10px] uppercase tracking-wider text-ink-700 bg-surface-900 border-b border-surface-800">
                <th class="text-right px-4 py-2 font-medium w-20">Port</th>
                <th class="text-left px-2 py-2 font-medium w-16">Proto</th>
                <th class="text-left px-2 py-2 font-medium">Owner</th>
                <th class="text-left px-4 py-2 font-medium">Process</th>
              </tr>
            </thead>
            <tbody class="font-mono text-xs">
              <For each={ports()}>
                {(p) => {
                  const owner = portOwner(p);
                  return (
                    <tr class="border-b border-surface-800/50 last:border-0 hover:bg-surface-800/40">
                      <td
                        class={`px-4 py-2 text-right ${p.proto === "udp" ? "text-warn" : "text-ok"}`}
                      >
                        {p.port}
                      </td>
                      <td class="px-2 py-2 uppercase text-ink-700">{p.proto}</td>
                      <td class="px-2 py-2">
                        <span
                          class={
                            owner.kind === "container"
                              ? "text-ok"
                              : owner.kind === "process"
                                ? "text-ink-100"
                                : "text-ink-700"
                          }
                        >
                          {owner.label}
                        </span>
                      </td>
                      <td class="px-4 py-2 text-ink-700">
                        {p.process ?? (owner.kind === "container" ? "docker-proxy" : "—")}
                      </td>
                    </tr>
                  );
                }}
              </For>
            </tbody>
          </table>
          {ports().length === 0 && (
            <div class="px-4 py-10 text-center text-sm text-ink-700">No active ports</div>
          )}
        </div>
      </section>
    </div>
  );
};

const MemBar: Component<{ label: string; used: number; total: number }> = (props) => {
  const pct = () => (props.total > 0 ? (props.used / props.total) * 100 : 0);
  return (
    <div class="mb-3 last:mb-0">
      <div class="flex justify-between text-xs mb-1">
        <span class="text-ink-100">{props.label}</span>
        <span class="font-mono text-ink-700">
          {fmtBytes(props.used)} / {fmtBytes(props.total, 0)} ({pct().toFixed(0)}%)
        </span>
      </div>
      <div class="h-2.5 bg-surface-800 rounded-full overflow-hidden">
        <div
          class="h-full rounded-full transition-all duration-500"
          style={{
            width: `${Math.min(pct(), 100)}%`,
            "background-color": pct() > 80 ? "rgb(var(--bad))" : pct() > 50 ? "rgb(var(--warn))" : "rgb(var(--ok))",
          }}
        />
      </div>
    </div>
  );
};

const ShowSwap: Component<{ used: number; total: number }> = (props) => (
  <Show when={props.total > 0}>
    <MemBar label="Swap" used={props.used} total={props.total} />
  </Show>
);
