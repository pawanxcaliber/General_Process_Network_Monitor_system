import { Component, Show } from "solid-js";
import { hostStore } from "@/stores/hostStore";
import { themeStore } from "@/stores/themeStore";
import { fmtBps } from "@/lib/format";
import { EngineIndicator } from "@/components/status/EngineIndicator";

export const Topbar: Component = () => {
  const host = () => hostStore.host();
  const cpu = () => host()?.cpu_percent ?? 0;
  const ramPct = () => {
    const h = host();
    if (!h || h.ram_total_bytes === 0) return 0;
    return (h.ram_used_bytes / h.ram_total_bytes) * 100;
  };
  const rx = () => {
    const h = host();
    return h?.history.net_rx_bps[h.history.net_rx_bps.length - 1] ?? 0;
  };
  const tx = () => {
    const h = host();
    return h?.history.net_tx_bps[h.history.net_tx_bps.length - 1] ?? 0;
  };
  const ping = () => host()?.ping_ms ?? null;
  const pingColor = () => {
    const p = ping();
    if (p == null) return "text-ink-700";
    return p >= 300 ? "text-bad" : p >= 100 ? "text-warn" : "text-ok";
  };

  return (
    <div class="flex items-center justify-between px-4 py-1.5 bg-surface-900/80 border-b border-surface-800">
      <EngineIndicator />
      <div class="flex items-center gap-5 text-[11px] font-mono">
        <span class="flex items-center gap-1.5">
          <span class="text-ink-700">CPU</span>
          <span class={cpu() > 80 ? "text-bad" : cpu() > 50 ? "text-warn" : "text-ok"}>
            {cpu().toFixed(0)}%
          </span>
        </span>
        <span class="flex items-center gap-1.5">
          <span class="text-ink-700">RAM</span>
          <span class={ramPct() > 80 ? "text-bad" : ramPct() > 50 ? "text-warn" : "text-ok"}>
            {ramPct().toFixed(0)}%
          </span>
        </span>
        <span class="flex items-center gap-1.5">
          <span class="text-ink-700">NET</span>
          <span class="text-ok">↓{fmtBps(rx())}</span>
          <span class="text-info">↑{fmtBps(tx())}</span>
        </span>
        <span class="flex items-center gap-1.5">
          <span class="text-ink-700">PING</span>
          <span class={pingColor()}>
            {ping() == null ? "—" : `${ping()!.toFixed(1)} ms`}
          </span>
        </span>
        <button
          onClick={() => themeStore.toggle()}
          title="Toggle theme"
          class="ml-1 w-7 h-7 flex items-center justify-center rounded-md border border-surface-800 text-ink-700 hover:text-ink-50 hover:border-surface-700 transition-colors"
        >
          <Show
            when={themeStore.theme() === "dark"}
            fallback={
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-3.5 h-3.5">
                <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
              </svg>
            }
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="w-3.5 h-3.5">
              <circle cx="12" cy="12" r="4" />
              <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" />
            </svg>
          </Show>
        </button>
      </div>
    </div>
  );
};
