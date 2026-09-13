import { createSignal } from "solid-js";
import type { RuntimeStatus } from "@/types";

const [runtimes, setRuntimes] = createSignal<RuntimeStatus[]>([]);

export const engineStore = {
  runtimes,
  setRuntimes,
};
