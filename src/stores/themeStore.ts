import { createSignal } from "solid-js";

type Theme = "dark" | "light";

function stored(): Theme {
  try {
    return localStorage.getItem("monitor-theme") === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

const [theme, setTheme] = createSignal<Theme>(stored());

function apply(t: Theme) {
  document.documentElement.classList.toggle("dark", t === "dark");
  try {
    localStorage.setItem("monitor-theme", t);
  } catch {
    // ignore
  }
}

apply(theme());

export const themeStore = {
  theme,
  toggle() {
    const next = theme() === "dark" ? "light" : "dark";
    setTheme(next);
    apply(next);
  },
};
