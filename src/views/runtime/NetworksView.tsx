import { Component, For, Show } from "solid-js";
import { useParams } from "@solidjs/router";
import { runtimeStore } from "@/stores/runtimeStore";
import type { RuntimeKind } from "@/types";
import { NotFound } from "@/views/NotFound";

const VALID: RuntimeKind[] = ["docker", "podman"];

export const NetworksView: Component = () => {
  const params = useParams<{ runtime: string }>();
  const kind = params.runtime as RuntimeKind;

  return (
    <Show when={VALID.includes(kind)} fallback={<NotFound label={params.runtime} />}>
      <NetworksInner kind={kind} />
    </Show>
  );
};

const NetworksInner: Component<{ kind: RuntimeKind }> = (props) => {
  const networks = () => runtimeStore.snap(props.kind).networks;

  return (
    <div class="p-5 space-y-4">
      <div>
        <h2 class="text-lg font-semibold capitalize">{props.kind} networks</h2>
        <p class="text-xs text-ink-700">Networks and attached containers</p>
      </div>

      <div class="bg-surface-900 border border-surface-800 rounded-lg overflow-hidden">
        <table class="w-full text-sm">
          <thead>
            <tr class="text-[10px] uppercase tracking-wider text-ink-700 border-b border-surface-800">
              <th class="text-left px-4 py-2.5 font-medium">Network</th>
              <th class="text-left px-2 py-2.5 font-medium">Driver</th>
              <th class="text-right px-2 py-2.5 font-medium">Containers</th>
              <th class="text-left px-4 py-2.5 font-medium">Attached</th>
            </tr>
          </thead>
          <tbody>
            <For each={networks()}>
              {(n) => (
                <tr class="border-b border-surface-800/50 last:border-0 hover:bg-surface-800/40">
                  <td class="px-4 py-2.5 font-mono text-ok">{n.name}</td>
                  <td class="px-2 py-2.5 font-mono text-xs text-ink-700">{n.driver}</td>
                  <td class="px-2 py-2.5 text-right font-mono">{n.containers.length}</td>
                  <td class="px-4 py-2.5">
                    <div class="flex gap-1.5 flex-wrap">
                      <For each={n.containers}>
                        {(name) => (
                          <span class="text-[10px] px-1.5 py-0.5 rounded bg-surface-800 text-ink-100 border border-surface-700 font-mono">
                            {name}
                          </span>
                        )}
                      </For>
                      {n.containers.length === 0 && (
                        <span class="text-xs text-ink-800">no containers attached</span>
                      )}
                    </div>
                  </td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
        {networks().length === 0 && (
          <div class="px-4 py-10 text-center text-sm text-ink-700">
            No networks found
          </div>
        )}
      </div>
    </div>
  );
};
