import { Component, For, Show } from "solid-js";
import { runtimeStore } from "@/stores/runtimeStore";
import { fmtBytes, stateColor } from "@/lib/format";
import { LineChart } from "@/components/charts/LineChart";

const GREEN = "#22c55e";

export const VmDashboard: Component = () => {
  const snap = () => runtimeStore.snap("vm");
  const running = () => snap().vms.filter((v) => v.state === "running");

  return (
    <div class="p-5 space-y-5">
      <div>
        <h2 class="text-lg font-semibold">Virtual Machines</h2>
        <p class="text-xs text-ink-700">{snap().detail ?? "—"}</p>
      </div>

      <div class="grid grid-cols-4 gap-4">
        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <span class="text-xs text-ink-700 uppercase tracking-wider">VMs</span>
          <div class="mt-2 flex items-baseline gap-2">
            <span class="text-3xl font-semibold text-warn">{running().length}</span>
            <span class="text-xs text-ink-700">running</span>
            <span class="text-2xl font-semibold text-ink-700">
              {snap().vms.length - running().length}
            </span>
            <span class="text-xs text-ink-700">stopped</span>
          </div>
        </div>
        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4 col-span-2">
          <span class="text-xs text-ink-700 uppercase tracking-wider">Allocated memory</span>
          <div class="text-3xl font-semibold mt-1">
            {fmtBytes(snap().vms.reduce((a, v) => a + v.memory_bytes, 0), 0)}
          </div>
        </div>
        <div class="bg-surface-900 border border-surface-800 rounded-lg p-4">
          <span class="text-xs text-ink-700 uppercase tracking-wider">Running trend</span>
          <LineChart
            series={[{ label: "running", color: GREEN, data: snap().history.cpu }]}
            height={72}
          />
        </div>
      </div>

      <Show
        when={snap().vms.length > 0}
        fallback={
          <div class="bg-surface-900 border border-surface-800 rounded-lg p-10 text-center">
            <div class="text-4xl mb-3 opacity-40">▭</div>
            <h3 class="text-sm font-semibold mb-1">No virtual machines found</h3>
            <p class="text-xs text-ink-700 max-w-md mx-auto">
              Hypervisor detection covers libvirt/KVM/QEMU, VirtualBox, VMware and Hyper-V.
              Create a VM or connect a hypervisor daemon to see it here.
            </p>
          </div>
        }
      >
        <div class="bg-surface-900 border border-surface-800 rounded-lg overflow-hidden">
          <table class="w-full text-sm">
            <thead>
              <tr class="text-[10px] uppercase tracking-wider text-ink-700 border-b border-surface-800">
                <th class="text-left px-4 py-2.5 font-medium">State</th>
                <th class="text-left px-2 py-2.5 font-medium">Name</th>
                <th class="text-left px-2 py-2.5 font-medium">Hypervisor</th>
                <th class="text-right px-2 py-2.5 font-medium">vCPUs</th>
                <th class="text-right px-4 py-2.5 font-medium">Memory</th>
              </tr>
            </thead>
            <tbody>
              <For each={snap().vms.sort((a, b) => a.name.localeCompare(b.name))}>
                {(vm) => (
                  <tr class="border-b border-surface-800/50 last:border-0 hover:bg-surface-800/40">
                    <td class="px-4 py-2.5">
                      <span
                        class="w-2 h-2 rounded-full inline-block"
                        style={{ "background-color": stateColor(vm.state === "running" ? "running" : "exited") }}
                        title={vm.state}
                      />
                      <span class="text-xs ml-2 text-ink-700">{vm.state}</span>
                    </td>
                    <td class="px-2 py-2.5 font-medium">{vm.name}</td>
                    <td class="px-2 py-2.5">
                      <span class="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/10 text-warn border border-amber-500/20 font-mono">
                        {vm.hypervisor}
                      </span>
                    </td>
                    <td class="px-2 py-2.5 text-right font-mono text-xs">{vm.vcpus || "—"}</td>
                    <td class="px-4 py-2.5 text-right font-mono text-xs">
                      {vm.memory_bytes > 0 ? fmtBytes(vm.memory_bytes, 0) : "—"}
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>
      </Show>
    </div>
  );
};
