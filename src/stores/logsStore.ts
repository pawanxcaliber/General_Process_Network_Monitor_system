import { createSignal } from "solid-js";
import type { LogLine } from "@/types";

const MAX_LINES = 2000;

const [logs, setLogs] = createSignal<Record<string, LogLine[]>>({});

export const logsStore = {
  logs,
  lines(id: string): LogLine[] {
    return logs()[id] ?? [];
  },
  append(id: string, line: LogLine) {
    setLogs((prev) => {
      const arr = [...(prev[id] ?? []), line];
      if (arr.length > MAX_LINES) {
        arr.splice(0, arr.length - MAX_LINES);
      }
      return { ...prev, [id]: arr };
    });
  },
  clear(id: string) {
    setLogs((prev) => ({ ...prev, [id]: [] }));
  },
};
