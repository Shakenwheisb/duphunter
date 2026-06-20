# DupHunter

A fully **local, offline** duplicate-file finder for **Windows 11**, built with
Tauri (Rust core + React/TypeScript + Fluent UI). It finds exact byte-identical
duplicates of any file type, plus **near-duplicate videos** — the same content
across different formats (e.g. `.mp4` vs `.mkv`) or different lengths.

> **Privacy:** Nothing ever leaves your computer. There are no network calls, no
> telemetry, and no cloud. The app's Content-Security-Policy grants no remote
> origins, and the Tauri capability set includes no HTTP permission.

---

## Features

- **Two clearly-separated detection modes**
  - **Exact** — a tiered pipeline that narrows candidates cheaply before doing
    expensive work: group by size → partial (head/middle/tail) hash → full-file
    hash → optional **paranoid** byte-for-byte verification.
  - **Video near-duplicate** — samples keyframes with ffmpeg, computes a
    perceptual **dHash** fingerprint per frame, and clusters videos by combined
    visual + duration + filename similarity (tunable threshold).
- **Pooled multi-folder scanning** — every file across every added folder is
  compared against every other, so duplicates are found *within and across*
  folders. Each copy shows which root it came from.
- **Hardlink awareness** — existing hardlinks are detected (via filesystem
  identity) and reported, but counted as **0 reclaimable** since deleting them
  frees nothing.
- **Safe, reversible actions** — Recycle Bin by default; permanent delete behind
  an explicit confirmation; move-to-quarantine; replace-with-hardlink/symlink.
  Every destructive action shows a **dry-run preview**, writes an **undo
  manifest**, and can never delete the chosen keeper or the last copy in a set.
- **Rich UI** — drag-and-drop folders, per-path exclude rules (globs,
  extensions, hidden/system, min/max size), a live scanning dashboard
  (phase indicator, throughput, ETA, running tallies, pause/resume/cancel),
  virtualized results, preview pane (image/video/audio), smart bulk-select
  (keep newest/oldest/shortest-path/preferred-folder), search/sort, and a live
  reclaimable-space summary.
- **Dark / light** themes that follow the OS setting; high-DPI ready.
- **Export** results to JSON or CSV.

---

## Requirements

- **Windows 11** (64-bit) for the shipped app. The codebase also builds and runs
  on macOS/Linux for development (the Recycle Bin becomes the OS Trash).
- **FFmpeg** (`ffmpeg` + `ffprobe`) on your `PATH` — required only for **video
  near-duplicate** mode. Exact mode needs nothing extra.
  - Install on Windows: `winget install Gyan.FFmpeg` (then restart DupHunter).
  - The app detects FFmpeg automatically and guides you if it's missing.

---

## Install (end users)

Grab the latest **`DupHunter_x64.msi`** (or the NSIS `.exe`) from the project's
GitHub Releases, run it, and launch DupHunter from the Start menu.

---

## Usage

1. **Add folders** — drag them onto the window or click to browse (multi-select).
2. **Pick a mode** — *Exact duplicates* or *Video near-duplicates* (set the
   similarity threshold and frames sampled).
3. Optionally set **exclude rules** and toggle **paranoid** verification.
4. **Start scan** — watch live progress; pause/resume/cancel any time.
5. In **Results**, review each set, choose what to keep (radio = keeper), use
   **Auto-keep** rules for bulk selection, then **Delete / Quarantine /
   Hardlink** — each with a dry-run preview and confirmation.
6. **Export** to JSON/CSV if you want a record.

**Keyboard shortcuts:** `Delete` → move selection to Recycle Bin · `1` keep
newest · `2` keep oldest · `3` keep shortest path.

---

## Development

```bash
# Prerequisites: Rust (stable), Node 18+, and (for video mode) ffmpeg on PATH.

npm install            # frontend deps
npm run tauri dev      # run the app (hot-reloads UI; rebuilds Rust on change)

# Build a production bundle (installer on Windows):
npm run tauri build
```

### Tests

```bash
cargo test -p dupcore   # core: size grouping, hashing, hardlinks, forced
                        # collision → paranoid split, dry-run safety, video math
npm test                # frontend smoke tests (Vitest + Testing Library)
```

The `ffmpeg_fingerprint_matches_reencode` test is skipped automatically when
FFmpeg isn't installed.

---

## Architecture

```
duphunter/
├── crates/dupcore/     # Pure-Rust engine (no GUI deps) — fully unit-tested
│   ├── model.rs        # serde types shared with the UI over IPC
│   ├── discovery.rs    # parallel walk + exclude rules + error collection
│   ├── identity.rs     # inode/file-index identity → hardlink detection
│   ├── hashing.rs      # streaming partial/full hash (BLAKE3/xxHash3/SHA-256)
│   ├── pipeline.rs     # tiered exact match + paranoid byte verification
│   ├── video.rs        # ffprobe metadata + dHash fingerprint + clustering
│   ├── actions.rs      # trash/permanent/quarantine/link with safety invariants
│   ├── manifest.rs     # undo-friendly action log
│   ├── cache.rs        # (path,size,mtime)->hash SQLite cache
│   └── export.rs       # JSON / CSV export
├── src-tauri/          # Thin Tauri shell: commands, events, background thread
├── src/                # React + Fluent UI frontend
│   ├── screens/        # ScanSetup, ScanningDashboard, Results
│   ├── components/     # SetCard, PreviewPane, ActionDialog, TitleBar
│   ├── store.ts        # Zustand state (config, scan lifecycle, selection)
│   └── api.ts          # typed wrapper over Tauri commands/events
└── .github/workflows/  # release.yml → Windows .msi / .exe
```

### Threading model

`start_scan` spawns the scan on a **background thread** so the UI never blocks.
Hashing fans out across CPU cores with `rayon`; all file I/O is streamed in
bounded chunks (whole files are never loaded into memory). Progress is pushed to
the UI as throttled `scan-progress` events, finishing with `scan-complete`.
**Pause / resume / cancel** flip an atomic on a shared `ScanControl` that workers
check at chunk boundaries — cancellation is prompt and leaves nothing in a broken
state (a scan only ever reads).

### How "duplicate" is decided

| Mode  | Signals |
|-------|---------|
| Exact | size pre-filter → sampled partial hash → full-file hash → optional byte-for-byte compare. Hardlinks identified by filesystem identity. |
| Video | ffprobe duration/resolution/codec + per-keyframe perceptual dHash, scored as `0.75·visual + 0.15·duration + 0.10·filename`, clustered above the chosen threshold. |

Near-match is **never** silently treated as exact-match — they are distinct modes.

---

## Notes & limits

- **Building the Windows installer requires Windows** (or the provided GitHub
  Actions workflow). A `.msi`/`.exe` cannot be produced from macOS/Linux.
- Zero-byte files are grouped but never auto-deleted.
- Future, out-of-scope-for-v1 ideas: audio acoustic fingerprinting (Chromaprint),
  text/document MinHash, archive content comparison. The `MatchMode` enum is
  designed to extend without rework.

## License

MIT.
