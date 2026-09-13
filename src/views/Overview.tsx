import { Component, For, Show } from "solid-js";
import { A } from "@solidjs/router";
import { hostStore } from "@/stores/hostStore";
import { runtimeStore } from "@/stores/runtimeStore";
import { fmtBps, fmtBytes, fmtMem, fmtPct } from "@/lib/format";
import { portOwner } from "@/lib/ports";
import { Sparkline } from "@/components/charts/Sparkline";
import { LineChart } from "@/components/charts/LineChart";

const GREEN = "#22c55e";
const BLUE = "#3b82f6";
const AMBER = "#f59e0b";
const RED = "#ef4444";
const PURPLE = "#a78bfa";

const RUNTIME_META: Array<{ kind: "docker" | "podman" | "kubernetes" | "vm"; label: string; href: string }> = [
  { kind: "docker", label: "Docker", href: "/docker" },
  { kind: "podman", label: "Podman", href: "/podman" },
  { kind: "kubernetes", label: "Kubernetes", href: "/kubernetes" },
  { kind: "vm", label: "VMs", href: "/vm" },
];

export const Overview: Component = () => {
  const host = () => hostStore.host();
  const hist = () => host()?.history;
  const last = (arr?: number[]) => (arr && arr.length > 0 ? arr[arr.length - 1] : 0);

  const ping = () => host()?.ping_ms ?? null;
  const pingColor = () => {
    const p = ping();
    if (p == null) return "text-ink-700";
    if (p > 100) return "text-bad";
    if (p > 40) return "text-warn";
    return "text-ok";
  };

  const temp = () => host()?.cpu_temp_c ?? null;
  const tempColor = () => {
    const t = temp();
    if (t == null) return "text-ink-700";
    if (t >= 80) return "text-bad";
    if (t >= 60) return "text-warn";
    return "text-ok";
  };

  return (
    <div class="p-5 space-y-5">
      <div>
        <h2 class="text-lg font-semibold">Overview</h2>
        <p class="text-xs text-ink-700">Everything running on this machine, at a glance</p>
      </div>

      <div class="grid grid-cols-6 gap-4">
        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <span class="text-xs text-ink-700 uppercase tracking-wider">Host CPU</span>
          <div class="text-3xl font-semibold mt-1">{fmtPct(host()?.cpu_percent ?? 0, 0)}</div>
          <Sparkline data={hist()?.cpu ?? []} color={GREEN} height={44} />
        </div>

        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <span class="text-xs text-ink-700 uppercase tracking-wider">Host Memory</span>
          <div class="text-xl font-semibold mt-1.5">
            {fmtBytes(host()?.ram_used_bytes ?? 0, 0)}
            <span class="text-xs text-ink-700"> / {fmtBytes(host()?.ram_total_bytes ?? 0, 0)}</span>
          </div>
          <Sparkline data={hist()?.ram_percent ?? []} color={BLUE} height={44} />
        </div>

        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <span class="text-xs text-ink-700 uppercase tracking-wider">Ping</span>
          <div class={`text-3xl font-semibold mt-1 ${pingColor()}`}>
            {ping() == null ? "—" : `${ping()!.toFixed(1)}`}
            <span class="text-sm font-normal text-ink-700"> ms</span>
          </div>
          <Sparkline data={hist()?.ping_ms ?? []} color={AMBER} height={44} />
        </div>

        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <span class="text-xs text-ink-700 uppercase tracking-wider">Temp</span>
          <div class={`text-3xl font-semibold mt-1 ${tempColor()}`}>
            {temp() == null ? "—" : `${temp()!.toFixed(1)}°`}
            <span class="text-sm font-normal text-ink-700">C</span>
          </div>
          <Sparkline data={hist()?.cpu_temp_c ?? []} color={RED} height={44} />
        </div>

        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <span class="text-xs text-ink-700 uppercase tracking-wider">iGPU</span>
          <Show
            when={host()?.gpu_igpu_present}
            fallback={
              <div class="text-2xl font-semibold mt-2 text-ink-700">—</div>
            }
          >
            <div class="text-2xl font-semibold mt-2">
              {fmtMem(host()?.gpu_igpu_used_bytes ?? 0)}
            </div>
          </Show>
          <Sparkline data={hist()?.gpu_igpu_used_bytes ?? []} color={GREEN} height={44} />
          <div class="text-[10px] text-ink-800 mt-3">integrated GPU memory</div>
        </div>

        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <span class="text-xs text-ink-700 uppercase tracking-wider">dGPU</span>
          <Show
            when={host()?.gpu_dgpu_present}
            fallback={
              <div class="text-2xl font-semibold mt-2 text-ink-700">—</div>
            }
          >
            <div class="text-2xl font-semibold mt-2">
              {fmtMem(host()?.gpu_dgpu_used_bytes ?? 0)}
            </div>
          </Show>
          <Sparkline data={hist()?.gpu_dgpu_used_bytes ?? []} color={PURPLE} height={44} />
          <div class="text-[10px] text-ink-800 mt-3">
            {host()?.gpu_dgpu_driver_unavailable
              ? "driver not loaded"
              : "discrete GPU memory"}
          </div>
        </div>
      </div>

      <div class="grid grid-cols-3 gap-4">
        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <h3 class="text-xs text-ink-700 uppercase tracking-wider mb-3">CPU utilization %</h3>
          <LineChart series={[{ label: "CPU %", color: GREEN, data: hist()?.cpu ?? [] }]} height={170} />
        </div>
        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <h3 class="text-xs text-ink-700 uppercase tracking-wider mb-3">Memory %</h3>
          <LineChart
            series={[{ label: "RAM %", color: BLUE, data: hist()?.ram_percent ?? [] }]}
            height={170}
          />
        </div>
        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <h3 class="text-xs text-ink-700 uppercase tracking-wider mb-3">Host network</h3>
          <LineChart
            series={[
              { label: "RX", color: GREEN, data: hist()?.net_rx_bps ?? [] },
              { label: "TX", color: BLUE, data: hist()?.net_tx_bps ?? [] },
            ]}
            height={170}
          />
        </div>
      </div>

      <div class="grid grid-cols-4 gap-4">
        <For each={RUNTIME_META}>
          {(meta) => {
            const snap = () => runtimeStore.snap(meta.kind);
            const detail = () => snap().detail ?? "not detected";
            const counts = () => {
              switch (meta.kind) {
                case "docker":
                case "podman": {
                  const running = snap().containers.filter((c) => c.state === "running").length;
                  return `${running}/${snap().containers.length} containers`;
                }
                case "kubernetes":
                  return `${snap().pods.length} pods · ${snap().services.length} services`;
                case "vm":
                  return `${snap().vms.length} VMs`;
              }
            };
            return (
              <A
                href={meta.href}
                class={`block bg-surface-900 border rounded-lg p-4 transition-colors ${
                  snap().available
                    ? "border-surface-800 hover:border-green-500/40"
                    : "border-surface-800/50 opacity-60"
                }`}
              >
                <div class="flex items-center justify-between">
                  <span class="text-sm font-medium">{meta.label}</span>
                  <span
                    class={`w-2 h-2 rounded-full ${
                      snap().available ? "bg-green-400" : "bg-surface-700"
                    }`}
                  />
                </div>
                <div class="text-xs text-ink-700 mt-2 font-mono">{counts()}</div>
                <div class="text-[10px] text-ink-800 mt-1 truncate">{detail()}</div>
              </A>
            );
          }}
        </For>
      </div>

      <Show when={host()}>
        <div class="grid grid-cols-2 gap-4">
          <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
            <h3 class="text-xs text-ink-700 uppercase tracking-wider mb-3">
              Listening ports ({host()?.ports.length ?? 0})
            </h3>
            <div class="max-h-[182px] overflow-y-auto font-mono text-xs">
              <For each={host()?.ports ?? []}>
                {(p) => {
                  const owner = portOwner(p);
                  return (
                    <div class="flex items-center gap-2 py-1 border-b border-surface-800/40 last:border-0">
                      <span class={`w-14 text-right shrink-0 ${p.proto === "udp" ? "text-warn" : "text-ok"}`}>
                        {p.port}
                      </span>
                      <span class="text-[9px] uppercase w-7 text-ink-800 shrink-0">{p.proto}</span>
                      <span
                        class={`truncate ${
                          owner.kind === "container"
                            ? "text-ok"
                            : owner.kind === "process"
                              ? "text-ink-100"
                              : "text-ink-700"
                        }`}
                      >
                        {owner.label}
                      </span>
                    </div>
                  );
                }}
              </For>
            </div>
            {(host()?.ports.length ?? 0) === 0 && (
              <div class="text-center text-ink-700 py-6 text-sm">No listening ports</div>
            )}
          </div>
          <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
            <h3 class="text-xs text-ink-700 uppercase tracking-wider mb-3">Disk I/O</h3>
            <LineChart
              series={[
                { label: "Read", color: "#a78bfa", data: hist()?.disk_read_bps ?? [] },
                { label: "Write", color: "#f59e0b", data: hist()?.disk_write_bps ?? [] },
              ]}
              height={140}
            />
            <div class="text-[10px] text-ink-800 mt-1 font-mono">
              R {fmtBps(last(hist()?.disk_read_bps))} · W {fmtBps(last(hist()?.disk_write_bps))}
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};
