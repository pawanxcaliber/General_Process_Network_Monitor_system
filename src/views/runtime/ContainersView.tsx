import { Component, For, Show, createMemo, createSignal } from "solid-js";
import { useNavigate, useParams, A } from "@solidjs/router";
import { runtimeStore } from "@/stores/runtimeStore";
import type { ContainerInfo, RuntimeKind } from "@/types";
import { fmtBps, fmtBytes, fmtPct, timeAgo, stateColor } from "@/lib/format";
import { NotFound } from "@/views/NotFound";

const VALID: RuntimeKind[] = ["docker", "podman"];

type Filter = "all" | "running" | "stopped";

export const ContainersView: Component = () => {
  const params = useParams<{ runtime: string }>();
  const kind = params.runtime as RuntimeKind;

  return (
    <Show when={VALID.includes(kind)} fallback={<NotFound label={params.runtime} />}>
      <ContainersInner kind={kind} />
    </Show>
  );
};

const ContainersInner: Component<{ kind: RuntimeKind }> = (props) => {
  const navigate = useNavigate();
  const [filter, setFilter] = createSignal<Filter>("all");
  const [search, setSearch] = createSignal("");

  const all = () => runtimeStore.snap(props.kind).containers;
  const runningCount = () => all().filter((c) => c.state === "running").length;
  const stoppedCount = () => all().length - runningCount();

  const list = createMemo(() => {
    const q = search().toLowerCase();
    return all()
      .filter((c) => {
        if (filter() === "running" && c.state !== "running") return false;
        if (filter() === "stopped" && c.state === "running") return false;
        if (q && !c.name.toLowerCase().includes(q) && !c.image.toLowerCase().includes(q))
          return false;
        return true;
      })
      .sort((a, b) => a.name.localeCompare(b.name));
  });

  return (
    <div class="p-5 space-y-4">
      <div class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold capitalize">{props.kind} containers</h2>
          <p class="text-xs text-ink-700">
            {all().length} total · {runningCount()} running · {stoppedCount()} stopped
          </p>
        </div>
        <input
          type="text"
          placeholder="Search by name or image…"
          value={search()}
          onInput={(e) => setSearch(e.currentTarget.value)}
          class="bg-surface-900 border border-surface-800 rounded-md px-3 py-1.5 text-sm w-64 placeholder:text-ink-700 focus:outline-none focus:border-green-500/50"
        />
      </div>

      <div class="flex gap-1 bg-surface-900 border border-surface-800 rounded-md p-1 w-fit">
        <FilterTab label={`All (${all().length})`} active={filter() === "all"} onClick={() => setFilter("all")} />
        <FilterTab label={`Running (${runningCount()})`} active={filter() === "running"} onClick={() => setFilter("running")} />
        <FilterTab label={`Stopped (${stoppedCount()})`} active={filter() === "stopped"} onClick={() => setFilter("stopped")} />
      </div>

      <div class="bg-surface-900 border border-surface-800 rounded-lg overflow-hidden">
        <table class="w-full text-sm">
          <thead>
            <tr class="text-[10px] uppercase tracking-wider text-ink-700 border-b border-surface-800">
              <th class="text-left px-4 py-2.5 font-medium">State</th>
              <th class="text-left px-2 py-2.5 font-medium">Name</th>
              <th class="text-left px-2 py-2.5 font-medium">Image</th>
              <th class="text-right px-2 py-2.5 font-medium">CPU %</th>
              <th class="text-left px-2 py-2.5 font-medium w-56">Memory</th>
              <th class="text-right px-2 py-2.5 font-medium">Net I/O</th>
              <th class="text-left px-2 py-2.5 font-medium">Ports</th>
              <th class="text-right px-4 py-2.5 font-medium">Status</th>
            </tr>
          </thead>
          <tbody>
            <For each={list()}>
              {(c) => (
                <ContainerRow
                  kind={props.kind}
                  container={c}
                  onOpen={() => navigate(`/${props.kind}/containers/${c.id}`)}
                />
              )}
            </For>
          </tbody>
        </table>
        {list().length === 0 && (
          <div class="px-4 py-10 text-center text-sm text-ink-700">
            No containers match the current filter
          </div>
        )}
      </div>
    </div>
  );
};

const FilterTab: Component<{ label: string; active: boolean; onClick: () => void }> = (props) => (
  <button
    onClick={props.onClick}
    class={`px-3 py-1 text-xs rounded transition-colors ${
      props.active ? "bg-surface-800 text-ink-50" : "text-ink-700 hover:text-ink-100"
    }`}
  >
    {props.label}
  </button>
);

const ContainerRow: Component<{
  kind: RuntimeKind;
  container: ContainerInfo;
  onOpen: () => void;
}> = (props) => {
  const c = () => props.container;
  const memPct = () =>
    c().memory_limit_bytes > 0 ? (c().memory_bytes / c().memory_limit_bytes) * 100 : 0;
  const isRunning = () => c().state === "running";

  return (
    <tr
      class="border-b border-surface-800/50 last:border-0 hover:bg-surface-800/40 cursor-pointer transition-colors"
      onClick={props.onOpen}
    >
      <td class="px-4 py-2.5">
        <span
          class={`w-2 h-2 rounded-full inline-block ${isRunning() ? "animate-pulse" : ""}`}
          style={{ "background-color": stateColor(c().state) }}
          title={c().state}
        />
      </td>
      <td class="px-2 py-2.5">
        <A
          href={`/${props.kind}/containers/${c().id}`}
          class="font-medium hover:text-ok"
          onClick={(e) => e.stopPropagation()}
        >
          {c().name}
        </A>
      </td>
      <td class="px-2 py-2.5 text-xs text-ink-700 font-mono max-w-[220px] truncate">
        {c().image}
      </td>
      <td class="px-2 py-2.5 text-right font-mono text-xs">
        {isRunning() ? fmtPct(c().cpu_percent) : "—"}
      </td>
      <td class="px-2 py-2.5">
        <div class="flex items-center gap-2">
          <span class="text-xs font-mono w-28 shrink-0">
            {isRunning() ? fmtBytes(c().memory_bytes) : "—"}
            <span class="text-ink-700"> / {fmtBytes(c().memory_limit_bytes, 0)}</span>
          </span>
          <div class="h-1.5 flex-1 bg-surface-800 rounded-full overflow-hidden">
            <div
              class="h-full rounded-full"
              style={{
                width: `${Math.min(memPct(), 100)}%`,
                "background-color":
                  memPct() > 80 ? "rgb(var(--bad))" : memPct() > 50 ? "rgb(var(--warn))" : "rgb(var(--ok))",
              }}
            />
          </div>
        </div>
      </td>
      <td class="px-2 py-2.5 text-right font-mono text-[11px]">
        <span class="text-ok">↓{fmtBps(c().rx_bps)}</span>
        <span class="text-info ml-1.5">↑{fmtBps(c().tx_bps)}</span>
      </td>
      <td class="px-2 py-2.5">
        <div class="flex gap-1 flex-wrap">
          <For each={c().ports.slice(0, 3)}>
            {(p) => (
              <span
                class={`text-[10px] font-mono px-1.5 py-0.5 rounded ${
                  p.host_port
                    ? "bg-green-500/10 text-ok border border-green-500/20"
                    : "bg-blue-500/10 text-info border border-blue-500/20"
                }`}
              >
                {p.host_port
                  ? `${p.host_port}:${p.container_port}`
                  : `${p.container_port}/${p.proto}`}
              </span>
            )}
          </For>
          {c().ports.length > 3 && (
            <span class="text-[10px] text-ink-700">+{c().ports.length - 3}</span>
          )}
        </div>
      </td>
      <td class="px-4 py-2.5 text-right">
        <div class="text-xs text-ink-700">{c().status}</div>
        <div class="text-[10px] text-ink-800">{timeAgo(c().created)}</div>
      </td>
    </tr>
  );
};
