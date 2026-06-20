// Small formatting/label helpers shared across the UI.

import type { Phase } from "./types";

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const v = bytes / Math.pow(1024, i);
  return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatDuration(secs: number): string {
  if (!isFinite(secs) || secs <= 0) return "0:00";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  const mm = m.toString().padStart(h > 0 ? 2 : 1, "0");
  const ss = s.toString().padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

export function formatDate(unixSecs: number): string {
  if (!unixSecs) return "—";
  return new Date(unixSecs * 1000).toLocaleString();
}

export function basename(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

export const PHASE_ORDER_EXACT: Phase[] = [
  "discovering",
  "grouping-by-size",
  "partial-hashing",
  "full-hashing",
  "verifying",
];

export const PHASE_ORDER_VIDEO: Phase[] = [
  "discovering",
  "probing",
  "sampling-frames",
  "fingerprinting",
  "clustering",
];

export const PHASE_LABELS: Record<Phase, string> = {
  discovering: "Discovering files",
  "grouping-by-size": "Grouping by size",
  "partial-hashing": "Partial hashing",
  "full-hashing": "Full hashing",
  verifying: "Verifying (paranoid)",
  probing: "Probing metadata",
  "sampling-frames": "Sampling frames",
  fingerprinting: "Fingerprinting",
  clustering: "Clustering",
  done: "Done",
};

const VIDEO_EXTS = new Set([
  "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg",
  "ts", "m2ts", "3gp", "ogv", "vob", "divx",
]);

export function isVideoPath(path: string): boolean {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return VIDEO_EXTS.has(ext);
}

const IMAGE_EXTS = new Set(["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "avif"]);
export function isImagePath(path: string): boolean {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return IMAGE_EXTS.has(ext);
}

const AUDIO_EXTS = new Set(["mp3", "flac", "wav", "aac", "ogg", "m4a", "wma", "opus"]);
export function isAudioPath(path: string): boolean {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return AUDIO_EXTS.has(ext);
}
