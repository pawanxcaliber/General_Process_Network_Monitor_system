import { runtimeStore } from "@/stores/runtimeStore";
import type { PortInfo } from "@/types";

export interface PortOwner {
  label: string;
  kind: "container" | "process" | "system";
}

export function portOwner(port: PortInfo): PortOwner {
  const snapshots = runtimeStore.snapshots();
  for (const kind of ["docker", "podman"] as const) {
    const snap = snapshots[kind];
    if (!snap) continue;
    for (const c of snap.containers) {
      if (c.state !== "running") continue;
      for (const p of c.ports) {
        if (p.host_port !== port.port) continue;
        if (port.proto && p.proto && port.proto !== p.proto) continue;
        return { label: `${kind}/${c.name}`, kind: "container" };
      }
    }
  }
  if (port.process) {
    return { label: port.process, kind: "process" };
  }
  return { label: "system", kind: "system" };
}
