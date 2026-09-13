import { Component } from "solid-js";

export const NotFound: Component<{ label: string }> = (props) => (
  <div class="h-full flex items-center justify-center">
    <div class="text-center">
      <div class="text-4xl mb-3">◌</div>
      <h2 class="text-lg font-semibold mb-1">"{props.label}" is not a known view</h2>
      <p class="text-xs text-ink-700">
        Supported runtimes: docker, podman, kubernetes, vm
      </p>
    </div>
  </div>
);
