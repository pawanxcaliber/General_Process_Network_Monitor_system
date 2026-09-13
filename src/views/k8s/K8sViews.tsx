import { Component, For, Show } from "solid-js";
import { runtimeStore } from "@/stores/runtimeStore";
import { fmtBytes, timeAgo } from "@/lib/format";

export const K8sDashboard: Component = () => {
  const snap = () => runtimeStore.snap("kubernetes");
  const running = () => snap().pods.filter((p) => p.phase.toLowerCase() === "running");
  const namespaces = () => new Set(snap().pods.map((p) => p.namespace)).size;
  const nodes = () => new Set(snap().pods.map((p) => p.node).filter(Boolean)).size;

  return (
    <div class="p-5 space-y-5">
      <div>
        <h2 class="text-lg font-semibold">Kubernetes</h2>
        <p class="text-xs text-ink-700">{snap().detail ?? "—"}</p>
      </div>

      <Show when={snap().available} fallback={<NotConnected />}>
        <div class="grid grid-cols-5 gap-4">
          <StatCard label="Pods running" value={String(running().length)} accent="text-grape" />
          <StatCard label="Pods total" value={String(snap().pods.length)} accent="" />
          <StatCard label="Services" value={String(snap().services.length)} accent="text-info" />
          <StatCard label="Namespaces" value={String(namespaces)} accent="" />
          <StatCard label="Nodes" value={String(nodes)} accent="" />
        </div>

        <div class="bg-surface-900 border border-surface-800 rounded-lg overflow-hidden">
          <div class="px-4 py-2.5 border-b border-surface-800 flex items-center justify-between">
            <h3 class="text-xs text-ink-700 uppercase tracking-wider">Pods</h3>
            <span class="text-[10px] font-mono text-ink-700">
              {snap().metrics_available ? "metrics-server: active" : "metrics-server: not detected"}
            </span>
          </div>
          <table class="w-full text-sm">
            <thead>
              <tr class="text-[10px] uppercase tracking-wider text-ink-700 border-b border-surface-800">
                <th class="text-left px-4 py-2 font-medium">Name</th>
                <th class="text-left px-2 py-2 font-medium">Namespace</th>
                <th class="text-left px-2 py-2 font-medium">Phase</th>
                <th class="text-right px-2 py-2 font-medium">Restarts</th>
                <th class="text-right px-2 py-2 font-medium">CPU</th>
                <th class="text-right px-2 py-2 font-medium">Memory</th>
                <th class="text-left px-4 py-2 font-medium">Node</th>
              </tr>
            </thead>
            <tbody>
              <For each={snap().pods.slice(0, 15)}>
                {(p) => (
                  <tr class="border-b border-surface-800/50 last:border-0">
                    <td class="px-4 py-2 font-medium">{p.name}</td>
                    <td class="px-2 py-2 font-mono text-xs text-ink-700">{p.namespace}</td>
                    <td class="px-2 py-2 text-xs">
                      <span
                        class={`px-1.5 py-0.5 rounded text-[10px] ${
                          p.phase.toLowerCase() === "running"
                            ? "bg-green-500/10 text-ok"
                            : p.phase.toLowerCase() === "pending"
                              ? "bg-amber-500/10 text-warn"
                              : "bg-surface-800 text-ink-700"
                        }`}
                      >
                        {p.phase}
                      </span>
                    </td>
                    <td class="px-2 py-2 text-right font-mono text-xs">{p.restarts}</td>
                    <td class="px-2 py-2 text-right font-mono text-xs">
                      {p.cpu_millis != null ? `${p.cpu_millis}m` : "—"}
                    </td>
                    <td class="px-2 py-2 text-right font-mono text-xs">
                      {p.mem_bytes != null ? fmtBytes(p.mem_bytes) : "—"}
                    </td>
                    <td class="px-4 py-2 font-mono text-xs text-ink-700 truncate max-w-[140px]">
                      {p.node || "—"}
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
          {snap().pods.length === 0 && (
            <div class="px-4 py-8 text-center text-sm text-ink-700">No pods found</div>
          )}
          {snap().pods.length > 15 && (
            <div class="px-4 py-2 text-[11px] text-ink-700 border-t border-surface-800">
              Showing 15 of {snap().pods.length} pods
            </div>
          )}
        </div>
      </Show>
    </div>
  );
};

export const StatCard: Component<{ label: string; value: string; accent: string }> = (props) => (
  <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
    <span class="text-xs text-ink-700 uppercase tracking-wider">{props.label}</span>
    <div class={`text-3xl font-semibold mt-1 ${props.accent}`}>{props.value}</div>
  </div>
);

export const NotConnected: Component = () => (
  <div class="bg-surface-900 border border-surface-800 rounded-lg p-10 text-center">
    <div class="text-4xl mb-3 opacity-40">◇</div>
    <h3 class="text-sm font-semibold mb-1">Kubernetes not connected</h3>
    <p class="text-xs text-ink-700 max-w-md mx-auto">
      No kubeconfig context found. Install a cluster (k3s, minikube, kind…) or point
      KUBECONFIG at an existing one.
    </p>
  </div>
);

export const K8sPods: Component = () => {
  const snap = () => runtimeStore.snap("kubernetes");

  return (
    <Show when={snap().available} fallback={<NotConnected />}>
      <div class="p-5 space-y-4">
        <div>
          <h2 class="text-lg font-semibold">Pods</h2>
          <p class="text-xs text-ink-700">{snap().pods.length} pods across all namespaces</p>
        </div>
        <div class="bg-surface-900 border border-surface-800 rounded-lg overflow-hidden">
          <table class="w-full text-sm">
            <thead>
              <tr class="text-[10px] uppercase tracking-wider text-ink-700 border-b border-surface-800">
                <th class="text-left px-4 py-2.5 font-medium">Namespace</th>
                <th class="text-left px-2 py-2.5 font-medium">Name</th>
                <th class="text-left px-2 py-2.5 font-medium">Phase</th>
                <th class="text-right px-2 py-2.5 font-medium">Restarts</th>
                <th class="text-right px-2 py-2.5 font-medium">Age</th>
                <th class="text-right px-2 py-2.5 font-medium">CPU</th>
                <th class="text-right px-4 py-2.5 font-medium">Memory</th>
              </tr>
            </thead>
            <tbody>
              <For each={[...snap().pods].sort((a, b) => a.namespace.localeCompare(b.namespace))}>
                {(p) => (
                  <tr class="border-b border-surface-800/50 last:border-0 hover:bg-surface-800/40">
                    <td class="px-4 py-2 font-mono text-xs text-grape">{p.namespace}</td>
                    <td class="px-2 py-2 font-medium">{p.name}</td>
                    <td class="px-2 py-2 text-xs text-ink-700">{p.phase}</td>
                    <td class="px-2 py-2 text-right font-mono text-xs">{p.restarts}</td>
                    <td class="px-2 py-2 text-right font-mono text-xs text-ink-700">
                      {p.created > 0 ? timeAgo(p.created) : "—"}
                    </td>
                    <td class="px-2 py-2 text-right font-mono text-xs">
                      {p.cpu_millis != null ? `${p.cpu_millis}m` : "—"}
                    </td>
                    <td class="px-4 py-2 text-right font-mono text-xs">
                      {p.mem_bytes != null ? fmtBytes(p.mem_bytes) : "—"}
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
          {snap().pods.length === 0 && (
            <div class="px-4 py-10 text-center text-sm text-ink-700">No pods found</div>
          )}
        </div>
      </div>
    </Show>
  );
};

export const K8sServices: Component = () => {
  const snap = () => runtimeStore.snap("kubernetes");

  return (
    <Show when={snap().available} fallback={<NotConnected />}>
      <div class="p-5 space-y-4">
        <div>
          <h2 class="text-lg font-semibold">Services</h2>
          <p class="text-xs text-ink-700">{snap().services.length} services</p>
        </div>
        <div class="bg-surface-900 border border-surface-800 rounded-lg overflow-hidden">
          <table class="w-full text-sm">
            <thead>
              <tr class="text-[10px] uppercase tracking-wider text-ink-700 border-b border-surface-800">
                <th class="text-left px-4 py-2.5 font-medium">Namespace</th>
                <th class="text-left px-2 py-2.5 font-medium">Name</th>
                <th class="text-left px-2 py-2.5 font-medium">Cluster IP</th>
                <th class="text-left px-4 py-2.5 font-medium">Ports</th>
              </tr>
            </thead>
            <tbody>
              <For each={snap().services}>
                {(s) => (
                  <tr class="border-b border-surface-800/50 last:border-0 hover:bg-surface-800/40">
                    <td class="px-4 py-2 font-mono text-xs text-grape">{s.namespace}</td>
                    <td class="px-2 py-2 font-medium">{s.name}</td>
                    <td class="px-2 py-2 font-mono text-xs text-info">{s.cluster_ip || "—"}</td>
                    <td class="px-4 py-2">
                      <div class="flex gap-1 flex-wrap">
                        <For each={s.ports}>
                          {(p) => (
                            <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-blue-500/10 text-info border border-blue-500/20">
                              {p}
                            </span>
                          )}
                        </For>
                      </div>
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
          {snap().services.length === 0 && (
            <div class="px-4 py-10 text-center text-sm text-ink-700">
              No services found
            </div>
          )}
        </div>
      </div>
    </Show>
  );
};
