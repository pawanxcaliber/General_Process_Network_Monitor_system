/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./src/**/*.{ts,tsx}", "./index.html"],
  theme: {
    extend: {
      colors: {
        surface: {
          50: "rgb(var(--s50) / <alpha-value>)",
          100: "rgb(var(--s100) / <alpha-value>)",
          700: "rgb(var(--s700) / <alpha-value>)",
          800: "rgb(var(--s800) / <alpha-value>)",
          900: "rgb(var(--s900) / <alpha-value>)",
          950: "rgb(var(--s950) / <alpha-value>)",
        },
        ink: {
          50: "rgb(var(--ink-50) / <alpha-value>)",
          100: "rgb(var(--ink-100) / <alpha-value>)",
          700: "rgb(var(--ink-700) / <alpha-value>)",
          800: "rgb(var(--ink-800) / <alpha-value>)",
        },
        ok: "rgb(var(--ok) / <alpha-value>)",
        info: "rgb(var(--info) / <alpha-value>)",
        warn: "rgb(var(--warn) / <alpha-value>)",
        bad: "rgb(var(--bad) / <alpha-value>)",
        grape: "rgb(var(--grape) / <alpha-value>)",
        edge: "rgb(var(--edge) / <alpha-value>)",
      },
    },
  },
  plugins: [],
};
