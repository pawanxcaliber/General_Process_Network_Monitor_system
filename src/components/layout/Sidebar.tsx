import { A } from "@solidjs/router";
import { Component, For, JSX, Show, createSignal } from "solid-js";
import { engineStore } from "@/stores/engineStore";
import type { RuntimeKind } from "@/types";

interface NavItem {
  href: string;
  label: string;
  icon: JSX.Element;
  exact?: boolean;
}

interface NavGroup {
  id: string;
  label: string;
  kind?: RuntimeKind;
  items: NavItem[];
}

function icon(svg: string): JSX.Element {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      class="w-4 h-4 shrink-0"
      innerHTML={svg}
    />
  );
}

const ICONS = {
  overview: icon('<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>'),
  host: icon('<rect x="6" y="6" width="12" height="12" rx="2"/><path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4"/>'),
  docker: icon('<path d="M21 10h-3V7h-3v3h-2V7h-3v3H8V7H5v3H2v4a5 5 0 0 0 5 5h9a5 5 0 0 0 5-5v-4z"/><path d="M5 7v3M8 7v3M11 7v3M14 7v3M18 10v3"/>'),
  podman: icon('<rect x="4" y="7" width="16" height="13" rx="2"/><path d="M8 7V5a4 4 0 0 1 8 0v2"/><path d="M4 12h16"/>'),
  kubernetes: icon('<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="3"/><path d="M12 3v6M12 15v6M3 12h6M15 12h6"/>'),
  vm: icon('<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>'),
  dash: icon('<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18M9 21V9"/>'),
  containers: icon('<path d="M21 8l-9-5-9 5v8l9 5 9-5V8z"/><path d="M3 8l9 5 9-5"/><path d="M12 13v8"/>'),
  networks: icon('<circle cx="12" cy="5" r="2.5"/><circle cx="5" cy="19" r="2.5"/><circle cx="19" cy="19" r="2.5"/><path d="M12 7.5v5M12 12.5L6.5 17M12 12.5l5.5 4.5"/>'),
  netmap: icon('<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c3 3.6 3 14.4 0 18M12 3c-3 3.6-3 14.4 0 18"/>'),
  map: icon('<polygon points="3 6 9 3 15 6 21 3 21 18 15 21 9 18 3 21"/><path d="M9 3v15M15 6v15"/>'),
  pods: icon('<ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v6c0 1.7 3.6 3 8 3s8-1.3 8-3V5"/><path d="M4 11v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6"/>'),
  services: icon('<circle cx="12" cy="12" r="3"/><path d="M12 2v4M12 18v4M2 12h4M18 12h4"/>'),
};

const OVERVIEW_GROUP: NavGroup = {
  id: "overview",
  label: "Overview",
  items: [
    { href: "/", label: "Overview", icon: ICONS.overview, exact: true },
  ],
};

const MONITORING_GROUP: NavGroup = {
  id: "monitoring",
  label: "Monitoring",
  items: [
    { href: "/host", label: "Host", icon: ICONS.host },
    { href: "/network", label: "Network Map", icon: ICONS.netmap },
  ],
};

function runtimeGroup(kind: RuntimeKind, name: string, groupIcon: JSX.Element): NavGroup {
  const base = `/${kind}`;
  const isContainerRuntime = kind === "docker" || kind === "podman";
  const items: NavItem[] = [
    { href: base, label: "Dashboard", icon: ICONS.dash },
  ];
  if (isContainerRuntime) {
    items.push(
      { href: `${base}/containers`, label: "Containers", icon: ICONS.containers },
      { href: `${base}/networks`, label: "Networks", icon: ICONS.networks },
      { href: `${base}/map`, label: "Map", icon: ICONS.map },
    );
  } else if (kind === "kubernetes") {
    items.push(
      { href: `${base}/pods`, label: "Pods", icon: ICONS.pods },
      { href: `${base}/services`, label: "Services", icon: ICONS.services },
      { href: `${base}/map`, label: "Map", icon: ICONS.map },
    );
  } else if (kind === "vm") {
    items.push({ href: `${base}/map`, label: "Map", icon: ICONS.map });
  }
  return { id: kind, label: name, kind, items: [{ href: base, label: name, icon: groupIcon, exact: true }, ...items] };
}

const SectionLabel: Component<{ label: string }> = (props) => (
  <div class="px-4 pt-4 pb-1 text-[10px] font-semibold uppercase tracking-widest text-ink-800">
    {props.label}
  </div>
);

const RuntimeSection: Component<{ group: NavGroup }> = (props) => {
  const [open, setOpen] = createSignal(true);
  const detail = () => {
    const rt = engineStore.runtimes().find((r) => r.kind === props.group.kind);
    return rt?.detail ?? "";
  };
  const subItems = () => props.group.items.slice(1);

  return (
    <div class="mb-0.5">
      <button
        onClick={() => setOpen((v) => !v)}
        class="w-full flex items-center gap-2.5 px-3 py-2 text-[13px] text-ink-700 hover:text-ink-100 hover:bg-surface-800/50 transition-colors"
      >
        <span class={`transition-transform ${open() ? "rotate-90" : ""}`}>▸</span>
        {props.group.items[0].icon}
        <span class="flex-1 text-left">{props.group.label}</span>
        <span class="w-1.5 h-1.5 rounded-full bg-green-400" />
      </button>
      <Show when={open()}>
        <For each={subItems()}>
          {(item) => (
            <A
              href={item.href}
              class="flex items-center gap-2.5 pl-11 pr-4 py-2 text-[12px] text-ink-700 hover:text-ink-100 hover:bg-surface-800/50 border-l-2 border-transparent transition-colors"
              activeClass="!text-ink-50 bg-surface-800/80 !border-green-500"
            >
              {item.icon}
              {item.label}
            </A>
          )}
        </For>
        <div class="pl-11 pr-4 pb-1 text-[9px] font-mono text-ink-800 truncate">{detail()}</div>
      </Show>
    </div>
  );
};

export const Sidebar: Component = () => {
  const runtimes = () => engineStore.runtimes();
  const isActive = (kind: RuntimeKind) => runtimes().find((r) => r.kind === kind)?.active ?? false;

  const dockerGroup = () => runtimeGroup("docker", "Docker", ICONS.docker);
  const podmanGroup = () => runtimeGroup("podman", "Podman", ICONS.podman);
  const k8sGroup = () => runtimeGroup("kubernetes", "Kubernetes", ICONS.kubernetes);
  const vmGroup = () => runtimeGroup("vm", "VMs", ICONS.vm);

  return (
    <aside class="w-52 shrink-0 h-full bg-surface-900 border-r border-surface-800 flex flex-col overflow-y-auto">
      <div class="px-4 py-4 border-b border-surface-800 flex items-center gap-2.5">
        <div class="w-7 h-7 rounded-md bg-green-500/15 border border-green-500/40 flex items-center justify-center">
          <span class="text-ok text-xs font-bold">M</span>
        </div>
        <span class="font-bold tracking-widest text-sm">MONITOR</span>
      </div>

      <nav class="flex-1 pb-2">
        <SectionLabel label="Overview" />
        <For each={OVERVIEW_GROUP.items}>
          {(item) => (
            <A
              href={item.href}
              end={item.exact}
              class="flex items-center gap-2.5 px-4 py-2.5 text-[13px] text-ink-700 hover:text-ink-100 hover:bg-surface-800/50 border-l-2 border-transparent transition-colors"
              activeClass="!text-ink-50 bg-surface-800/80 !border-green-500"
            >
              {item.icon}
              {item.label}
            </A>
          )}
        </For>

        <SectionLabel label="Monitoring" />
        <For each={MONITORING_GROUP.items}>
          {(item) => (
            <A
              href={item.href}
              class="flex items-center gap-2.5 px-4 py-2.5 text-[13px] text-ink-700 hover:text-ink-100 hover:bg-surface-800/50 border-l-2 border-transparent transition-colors"
              activeClass="!text-ink-50 bg-surface-800/80 !border-green-500"
            >
              {item.icon}
              {item.label}
            </A>
          )}
        </For>

        <SectionLabel label="Runtimes" />
        <Show when={isActive("docker")}>
          <RuntimeSection group={dockerGroup()} />
        </Show>
        <Show when={isActive("podman")}>
          <RuntimeSection group={podmanGroup()} />
        </Show>
        <Show when={isActive("kubernetes")}>
          <RuntimeSection group={k8sGroup()} />
        </Show>
        <Show when={isActive("vm")}>
          <RuntimeSection group={vmGroup()} />
        </Show>
        <Show when={runtimes().length > 0 && !runtimes().some((r) => r.active)}>
          <div class="px-4 py-3 text-[11px] text-ink-800">
            No container runtimes or hypervisors detected on this system.
          </div>
        </Show>
      </nav>

      <div class="px-4 py-3 text-[10px] text-ink-800 border-t border-surface-800">
        Container &amp; Host Monitor
      </div>
    </aside>
  );
};
