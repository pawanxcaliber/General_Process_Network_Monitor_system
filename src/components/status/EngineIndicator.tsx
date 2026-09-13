import { Component } from "solid-js";
import { engineStore } from "@/stores/engineStore";

export const EngineIndicator: Component = () => {
  return (
    <div class="flex items-center gap-2">
      {engineStore.runtimes().map((rt) => (
        <div
          class={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium transition-colors ${
            rt.active
              ? "bg-green-500/10 text-ok border border-green-500/20"
              : "bg-surface-800 text-ink-700 border border-surface-800"
          }`}
          title={rt.detail ?? ""}
        >
          <span
            class={`w-1.5 h-1.5 rounded-full ${rt.active ? "bg-green-400" : "bg-surface-700"}`}
          />
          {rt.name}
        </div>
      ))}
    </div>
  );
};
