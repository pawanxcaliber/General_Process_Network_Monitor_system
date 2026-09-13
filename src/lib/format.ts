export function fmtBytes(n: number, digits = 1): string {
  if (!isFinite(n) || n <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  const i = Math.max(0, Math.min(units.length - 1, Math.floor(Math.log(n) / Math.log(1024))));
  return `${(n / 1024 ** i).toFixed(i === 0 ? 0 : digits)} ${units[i]}`;
}

/// Memory formatting: below 1 MB in KB, at/above 1 MB with decimals.
export function fmtMem(n: number): string {
  if (!isFinite(n) || n <= 0) return "0 B";
  const KB = 1024;
  const MB = KB * 1024;
  const GB = MB * 1024;
  if (n < KB) return `${Math.round(n)} B`;
  if (n < MB) return `${Math.round(n / KB)} KB`;
  if (n < GB) return `${(n / MB).toFixed(1)} MB`;
  return `${(n / GB).toFixed(2)} GB`;
}

export function fmtBps(n: number): string {
  return `${fmtBytes(n)}/s`;
}

export function fmtPct(n: number, digits = 1): string {
  return `${(isFinite(n) ? n : 0).toFixed(digits)}%`;
}

export function timeAgo(unix: number): string {
  const s = Math.max(0, Math.floor(Date.now() / 1000 - unix));
  if (s < 60) return `${s}s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.floor(h / 24);
  if (d < 30) return `${d}d ago`;
  const mo = Math.floor(d / 30);
  return `${mo}mo ago`;
}

export function stateColor(state: string): string {
  switch (state) {
    case "running":
      return "#22c55e";
    case "paused":
      return "#f59e0b";
    case "restarting":
      return "#f97316";
    default:
      return "#64748b";
  }
}

export function shortId(id: string): string {
  return id.slice(0, 12);
}
