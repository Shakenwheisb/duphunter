import { create } from "zustand";
import {
  type ActionTarget,
  type DupSet,
  type MatchMode,
  type Progress,
  type ScanConfig,
  type ScanResult,
  defaultConfig,
} from "./types";

export type View = "setup" | "scanning" | "results";
export type BulkRule = "newest" | "oldest" | "shortest-path" | "preferred-folder";

const RECENTS_KEY = "duphunter.recents";

function loadRecents(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECENTS_KEY) ?? "[]");
  } catch {
    return [];
  }
}

interface AppStore {
  view: View;
  config: ScanConfig;
  ffmpegAvailable: boolean;
  progress: Progress | null;
  result: ScanResult | null;
  /** Working copy of sets with live keep/remove roles. */
  sets: DupSet[];
  error: string | null;
  recents: string[];

  setView: (v: View) => void;
  setFfmpeg: (ok: boolean) => void;
  patchConfig: (p: Partial<ScanConfig>) => void;
  setMode: (m: MatchMode) => void;
  addRoots: (paths: string[]) => void;
  removeRoot: (path: string) => void;

  beginScan: () => void;
  setProgress: (p: Progress) => void;
  finishScan: (r: ScanResult) => void;
  setError: (e: string | null) => void;

  setKeeper: (setId: number, path: string) => void;
  toggleRemove: (setId: number, path: string) => void;
  keepAll: (setId: number) => void;
  applyBulk: (rule: BulkRule, preferredRoot?: string) => void;

  removalTargets: () => ActionTarget[];
  reclaimableSelected: () => number;
  removalCount: () => number;
}

export const useStore = create<AppStore>((set, get) => ({
  view: "setup",
  config: defaultConfig(),
  ffmpegAvailable: false,
  progress: null,
  result: null,
  sets: [],
  error: null,
  recents: loadRecents(),

  setView: (v) => set({ view: v }),
  setFfmpeg: (ok) => set({ ffmpegAvailable: ok }),
  patchConfig: (p) => set((s) => ({ config: { ...s.config, ...p } })),
  setMode: (m) => set((s) => ({ config: { ...s.config, mode: m } })),

  addRoots: (paths) =>
    set((s) => {
      const roots = Array.from(new Set([...s.config.roots, ...paths]));
      const recents = Array.from(new Set([...paths, ...s.recents])).slice(0, 8);
      localStorage.setItem(RECENTS_KEY, JSON.stringify(recents));
      return { config: { ...s.config, roots }, recents };
    }),
  removeRoot: (path) =>
    set((s) => ({
      config: { ...s.config, roots: s.config.roots.filter((r) => r !== path) },
    })),

  beginScan: () =>
    set({ view: "scanning", progress: null, result: null, sets: [], error: null }),
  setProgress: (p) => set({ progress: p }),
  finishScan: (r) =>
    set({ result: r, sets: r.sets, view: "results" }),
  setError: (e) => set({ error: e, view: e ? "setup" : get().view }),

  // Radio-style: choose the single keeper; all others become removals.
  setKeeper: (setId, path) =>
    set((s) => ({
      sets: s.sets.map((d) =>
        d.id !== setId
          ? d
          : {
              ...d,
              members: d.members.map((m) => ({
                ...m,
                role: m.entry.path === path ? "keep" : "remove",
              })),
            }
      ),
    })),

  toggleRemove: (setId, path) =>
    set((s) => ({
      sets: s.sets.map((d) => {
        if (d.id !== setId) return d;
        const members = d.members.map((m) =>
          m.entry.path === path
            ? { ...m, role: m.role === "remove" ? ("keep" as const) : ("remove" as const) }
            : m
        );
        // Safety: never allow a set with zero keepers.
        if (!members.some((m) => m.role === "keep")) return d;
        return { ...d, members };
      }),
    })),

  keepAll: (setId) =>
    set((s) => ({
      sets: s.sets.map((d) =>
        d.id !== setId
          ? d
          : { ...d, members: d.members.map((m) => ({ ...m, role: "keep" as const })) }
      ),
    })),

  // Auto-select all-but-one per set by a rule, always keeping exactly one.
  applyBulk: (rule, preferredRoot) =>
    set((s) => ({
      sets: s.sets.map((d) => {
        if (d.zeroByte) return d; // never auto-touch zero-byte sets
        const keeper = pickKeeper(d, rule, preferredRoot);
        return {
          ...d,
          members: d.members.map((m) => ({
            ...m,
            role: m.entry.path === keeper ? "keep" : "remove",
          })),
        };
      }),
    })),

  removalTargets: () => {
    const targets: ActionTarget[] = [];
    for (const d of get().sets) {
      const keeper = d.members.find((m) => m.role === "keep");
      const remove = d.members.filter((m) => m.role === "remove").map((m) => m.entry.path);
      if (keeper && remove.length > 0) {
        targets.push({ setId: d.id, keep: keeper.entry.path, remove });
      }
    }
    return targets;
  },

  reclaimableSelected: () => {
    let total = 0;
    for (const d of get().sets) {
      if (d.zeroByte) continue;
      for (const m of d.members) {
        // Hardlinked copies free no space when removed.
        if (m.role === "remove" && !m.isHardlinkOfOther) total += m.entry.size;
      }
    }
    return total;
  },

  removalCount: () =>
    get().sets.reduce(
      (n, d) => n + d.members.filter((m) => m.role === "remove").length,
      0
    ),
}));

/** Decide which member to keep for a set, given a bulk rule. */
function pickKeeper(d: DupSet, rule: BulkRule, preferredRoot?: string): string {
  const members = [...d.members];
  switch (rule) {
    case "newest":
      members.sort((a, b) => b.entry.mtime - a.entry.mtime);
      break;
    case "oldest":
      members.sort((a, b) => a.entry.mtime - b.entry.mtime);
      break;
    case "shortest-path":
      members.sort((a, b) => a.entry.path.length - b.entry.path.length);
      break;
    case "preferred-folder": {
      const inPref = (p: string) => (preferredRoot && p.startsWith(preferredRoot) ? 0 : 1);
      members.sort((a, b) => inPref(a.entry.path) - inPref(b.entry.path));
      break;
    }
  }
  return members[0].entry.path;
}
