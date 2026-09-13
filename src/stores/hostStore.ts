import { createSignal } from "solid-js";
import type { HostPayload } from "@/types";

const [host, setHost] = createSignal<HostPayload | null>(null);

export const hostStore = {
  host,
  setHost,
};
