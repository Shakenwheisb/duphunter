// Thin wrapper around the Tauri command/event surface. Every backend call goes
// through here so components stay decoupled from the IPC details, and so the app
// degrades gracefully when run outside Tauri (e.g. plain `vite` or unit tests).

import type {
  ActionPlan,
  ActionReport,
  DupSet,
  Progress,
  ScanConfig,
  ScanResult,
} from "./types";

/** True when running inside the Tauri shell (vs. a plain browser/test env). */
export const inTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export async function checkFfmpeg(): Promise<boolean> {
  if (!inTauri()) return false;
  return invoke<boolean>("check_ffmpeg");
}

export async function startScan(config: ScanConfig): Promise<void> {
  return invoke<void>("start_scan", { config });
}

export const pauseScan = () => invoke<void>("pause_scan");
export const resumeScan = () => invoke<void>("resume_scan");
export const cancelScan = () => invoke<void>("cancel_scan");

export async function runAction(plan: ActionPlan): Promise<ActionReport> {
  return invoke<ActionReport>("run_action", { plan });
}

export async function exportResults(
  sets: DupSet[],
  path: string,
  format: "json" | "csv"
): Promise<void> {
  return invoke<void>("export_results", { sets, path, format });
}

export async function videoThumbnail(path: string): Promise<string> {
  return invoke<string>("video_thumbnail", { path });
}

/** Subscribe to live scan progress. Returns an unsubscribe function. */
export async function onProgress(cb: (p: Progress) => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  const un = await listen<Progress>("scan-progress", (e) => cb(e.payload));
  return un;
}

export async function onComplete(cb: (r: ScanResult) => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  const un = await listen<{ result: ScanResult }>("scan-complete", (e) =>
    cb(e.payload.result)
  );
  return un;
}

export async function onError(cb: (msg: string) => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  const un = await listen<string>("scan-error", (e) => cb(e.payload));
  return un;
}

/** Native multi-folder picker. Returns [] when cancelled or not in Tauri. */
export async function pickFolders(): Promise<string[]> {
  if (!inTauri()) return [];
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, multiple: true, recursive: false });
  if (!picked) return [];
  return Array.isArray(picked) ? picked : [picked];
}

/** Native save dialog for exports. */
export async function pickSavePath(
  defaultName: string
): Promise<string | null> {
  if (!inTauri()) return null;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({ defaultPath: defaultName });
}

/** Pick a single folder (for quarantine destination). */
export async function pickFolder(): Promise<string | null> {
  if (!inTauri()) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const p = await open({ directory: true, multiple: false });
  return typeof p === "string" ? p : null;
}

/** Convert a filesystem path to an asset URL usable in <img>/<video>. */
export async function assetUrl(path: string): Promise<string> {
  if (!inTauri()) return path;
  const { convertFileSrc } = await import("@tauri-apps/api/core");
  return convertFileSrc(path);
}

/** Reveal a file in the OS file manager. */
export async function revealInExplorer(path: string): Promise<void> {
  if (!inTauri()) return;
  const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
  await revealItemInDir(path);
}
