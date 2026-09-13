/* @refresh reload */
import { render } from "solid-js/web";
import { Router, Route } from "@solidjs/router";
import { AppShell } from "./components/layout/AppShell";
import { Overview } from "./views/Overview";
import { Host } from "./views/Host";
import { RuntimeDashboard } from "./views/runtime/RuntimeDashboard";
import { ContainersView } from "./views/runtime/ContainersView";
import { ContainerDetail } from "./views/runtime/ContainerDetail";
import { NetworksView } from "./views/runtime/NetworksView";
import { TopologyMap } from "./views/runtime/TopologyMap";
import { K8sDashboard, K8sPods, K8sServices } from "./views/k8s/K8sViews";
import { VmDashboard } from "./views/VmDashboard";
import { NetworkMap } from "./views/network/NetworkMap";
import "./styles/global.css";
import "uplot/dist/uPlot.min.css";

const root = document.getElementById("root");

render(
  () => (
    <Router root={AppShell}>
      <Route path="/" component={Overview} />
      <Route path="/host" component={Host} />
      <Route path="/network" component={NetworkMap} />

      <Route path="/:runtime" component={RuntimeDashboard} />
      <Route path="/:runtime/containers" component={ContainersView} />
      <Route path="/:runtime/containers/:id" component={ContainerDetail} />
      <Route path="/:runtime/networks" component={NetworksView} />
      <Route path="/:runtime/map" component={TopologyMap} />

      <Route path="/kubernetes" component={K8sDashboard} />
      <Route path="/kubernetes/pods" component={K8sPods} />
      <Route path="/kubernetes/services" component={K8sServices} />
      <Route path="/kubernetes/map" component={TopologyMap} />

      <Route path="/vm" component={VmDashboard} />
      <Route path="/vm/map" component={TopologyMap} />
    </Router>
  ),
  root!,
);
