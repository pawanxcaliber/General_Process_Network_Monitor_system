import { Component } from "solid-js";
import type { RouteSectionProps } from "@solidjs/router";
import { Sidebar } from "./Sidebar";
import { Topbar } from "./Topbar";
import { useBackend } from "@/hooks/useBackend";

export const AppShell: Component<RouteSectionProps> = (props) => {
  useBackend();

  return (
    <div class="h-screen w-screen flex overflow-hidden bg-surface-950 text-ink-50">
      <Sidebar />
      <div class="flex-1 flex flex-col overflow-hidden">
        <Topbar />
        <main class="flex-1 overflow-auto">{props.children}</main>
      </div>
    </div>
  );
};
