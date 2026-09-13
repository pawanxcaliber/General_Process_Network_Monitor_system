import { Component, Show, createEffect, createMemo, createSignal } from "solid-js";
import { useParams } from "@solidjs/router";
import { runtimeStore } from "@/stores/runtimeStore";
import { TopologyCanvas } from "@/components/topology/TopologyCanvas";
import { TopologyNodePanel } from "@/components/topology/TopologyNodePanel";
import { NotFound } from "@/views/NotFound";
import type { RuntimeKind } from "@/types";

const VALID: RuntimeKind[] = ["docker", "podman", "kubernetes", "vm"];

export const TopologyMap: Component = () => {
  const params = useParams<{ runtime: string }>();
  const kind = params.runtime as RuntimeKind;
  const [selected, setSelected] = createSignal<string | null>(null);
  const [paused, setPaused] = createSignal(false);
  let latest: ReturnType<typeof runtimeStore.snap> | null = null;

  createEffect(() => {
    void params.runtime;
    setSelected(null);
    setPaused(false);
    latest = null;
  });

  const snap = createMemo(() => {
    const s = runtimeStore.snap(kind);
    if (!paused()) {
      latest = s;
      return s;
    }
    return latest ?? s;
  });

  const origin = createMemo(() => {
    const map = new Map<string, string>();
    for (const n of snap().nodes) map.set(n.id, kind);
    return map;
  });

  const selectedNode = createMemo(() => snap().nodes.find((n) => n.id === selected()));

  return (
    <Show when={VALID.includes(kind)} fallback={<NotFound label={params.runtime} />}>
      <div class="h-full flex flex-col">
        <div class="px-5 pt-4 pb-2 flex items-baseline gap-3">
          <h2 class="text-lg font-semibold capitalize">{kind} map</h2>
          <p class="text-xs text-ink-700">Edge pulse speed reflects live traffic</p>
          <div class="ml-auto flex items-center gap-2">
            <Show when={paused()}>
              <span class="text-[10px] font-mono text-warn">❄ Data frozen</span>
            </Show>
            <button
              onClick={() => setPaused(!paused())}
              class={`text-[11px] px-2.5 py-0.5 rounded border transition-colors ${
                paused()
                  ? "border-amber-500/40 text-warn bg-amber-500/10"
                  : "border-surface-800 text-ink-700 hover:text-ink-100"
              }`}
            >
              {paused() ? "Resume" : "Freeze"}
            </button>
          </div>
        </div>
        <div class="flex-1 min-h-0 flex">
          <div class="flex-1 min-h-0 flex flex-col">
            <TopologyCanvas
              nodes={snap().nodes}
              edges={snap().edges}
              nodeOrigin={origin()}
              selectedId={selected()}
              onSelect={setSelected}
              paused={paused()}
            />
          </div>
          <Show when={selectedNode()}>
            {(n) => (
              <TopologyNodePanel
                node={n()}
                edges={snap().edges}
                nodes={snap().nodes}
                origin={kind}
                onClose={() => setSelected(null)}
              />
            )}
          </Show>
        </div>
      </div>
    </Show>
  );
};

