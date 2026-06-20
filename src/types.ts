// TypeScript mirrors of the `dupcore` serde types. Field names are camelCase and
// enum values match the Rust kebab-case serialization, so these cross the Tauri
// IPC boundary without any translation layer.

export type HashAlgo = "blake3" | "xxh3" | "sha256";
export type MatchMode = "exact" | "video-near-dup";
export type SymlinkPolicy = "skip" | "follow";
export type MemberRole = "keep" | "remove";
export type ActionKind =
  | "recycle-bin"
  | "permanent-delete"
  | "quarantine"
  | "hardlink"
  | "symlink";

export type Phase =
  | "discovering"
  | "grouping-by-size"
  | "partial-hashing"
  | "full-hashing"
  | "verifying"
  | "probing"
  | "sampling-frames"
  | "fingerprinting"
  | "clustering"
  | "done";

export interface ExcludeRules {
  excludeDirs: string[];
  globPatterns: string[];
  excludeExtensions: string[];
  skipHiddenSystem: boolean;
  minSize: number | null;
  maxSize: number | null;
}

export interface VideoOptions {
  similarityThreshold: number;
  frameSamples: number;
}

export interface ScanConfig {
  roots: string[];
  excludes: ExcludeRules;
  mode: MatchMode;
  hashAlgo: HashAlgo;
  paranoid: boolean;
  symlinks: SymlinkPolicy;
  video: VideoOptions;
}

export interface FileId {
  volume: number;
  fileId: number;
}

export interface VideoMeta {
  durationSecs: number;
  width: number;
  height: number;
  videoCodec: string;
  audioCodec: string;
  bitrate: number;
}

export interface FileEntry {
  path: string;
  root: string;
  size: number;
  mtime: number;
  identity: FileId | null;
  video?: VideoMeta | null;
}

export interface DupMember {
  entry: FileEntry;
  role: MemberRole;
  isHardlinkOfOther: boolean;
}

export interface DupSet {
  id: number;
  mode: MatchMode;
  members: DupMember[];
  hash: string | null;
  similarity: number | null;
  reclaimable: number;
  zeroByte: boolean;
}

export interface ScanIssue {
  path: string;
  message: string;
}

export interface Progress {
  phase: Phase;
  filesDone: number;
  filesTotal: number;
  bytesDone: number;
  bytesTotal: number;
  currentPath: string | null;
  dupSets: number;
  reclaimable: number;
  elapsedSecs: number;
}

export interface ScanResult {
  sets: DupSet[];
  issues: ScanIssue[];
  filesScanned: number;
  bytesScanned: number;
  elapsedSecs: number;
  cancelled: boolean;
}

export interface ActionTarget {
  setId: number;
  keep: string;
  remove: string[];
}

export interface ActionPlan {
  kind: ActionKind;
  dryRun: boolean;
  quarantineDir: string | null;
  targets: ActionTarget[];
}

export interface ActionItemResult {
  setId: number;
  source: string;
  destination: string | null;
  bytes: number;
  ok: boolean;
  message: string | null;
}

export interface ActionReport {
  kind: ActionKind;
  dryRun: boolean;
  items: ActionItemResult[];
  totalFiles: number;
  totalBytes: number;
  manifestPath: string | null;
}

export const defaultExcludes = (): ExcludeRules => ({
  excludeDirs: [],
  globPatterns: [],
  excludeExtensions: [],
  skipHiddenSystem: true,
  minSize: null,
  maxSize: null,
});

export const defaultConfig = (): ScanConfig => ({
  roots: [],
  excludes: defaultExcludes(),
  mode: "exact",
  hashAlgo: "blake3",
  paranoid: false,
  symlinks: "skip",
  video: { similarityThreshold: 88, frameSamples: 5 },
});
