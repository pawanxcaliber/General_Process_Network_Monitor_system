import { createSignal } from "solid-js";
import type { NetworkMapPayload } from "@/types";

const [map, setMap] = createSignal<NetworkMapPayload | null>(null);

export const networkMapStore = {
  map,
  setMap,
};
