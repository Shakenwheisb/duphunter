import { useEffect, useMemo, useRef, useState } from "react";
import {
  Badge,
  Body1,
  Button,
  Caption1,
  Dropdown,
  Input,
  makeStyles,
  MessageBar,
  MessageBarBody,
  Option,
  Subtitle1,
  Toolbar,
  ToolbarButton,
  ToolbarDivider,
  tokens,
} from "@fluentui/react-components";
import {
  ArrowExportRegular,
  ArrowLeftRegular,
  CheckmarkCircleRegular,
  DeleteRegular,
  FolderLinkRegular,
  ArchiveRegular,
} from "@fluentui/react-icons";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useStore, type BulkRule } from "../store";
import * as api from "../api";
import type { ActionKind, ActionReport, DupSet, FileEntry } from "../types";
import { PreviewPane } from "../components/PreviewPane";
import { ActionDialog } from "../components/ActionDialog";
import { SetCard } from "../components/SetCard";
import { formatBytes } from "../util";

const useStyles = makeStyles({
  layout: { flex: 1, display: "flex", minHeight: 0 },
  main: { flex: 1, display: "flex", flexDirection: "column", minWidth: 0 },
  summary: {
    display: "flex",
    gap: "24px",
    padding: "12px 18px",
    borderBottom: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: tokens.colorNeutralBackground1,
    alignItems: "center",
  },
  summaryItem: { display: "flex", flexDirection: "column" },
  summaryValue: { fontSize: "18px", fontWeight: 700, lineHeight: 1.1 },
  toolbar: { padding: "6px 12px", borderBottom: `1px solid ${tokens.colorNeutralStroke2}`, flexWrap: "wrap" },
  list: { flex: 1, overflowY: "auto", padding: "12px 16px" },
  empty: {
    flex: 1,
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    gap: "10px",
    opacity: 0.8,
  },
  grow: { flex: 1 },
});

type SortKey = "reclaimable" | "size" | "count" | "path";

export function Results() {
  const s = useStyles();
  const sets = useStore((st) => st.sets);
  const result = useStore((st) => st.result);
  const setView = useStore((st) => st.setView);
  const applyBulk = useStore((st) => st.applyBulk);
  const removalCount = useStore((st) => st.removalCount);
  const reclaimableSelected = useStore((st) => st.reclaimableSelected);

  const [selected, setSelected] = useState<FileEntry | null>(null);
  const [search, setSearch] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("reclaimable");
  const [dialogKind, setDialogKind] = useState<ActionKind | null>(null);
  const [lastReport, setLastReport] = useState<ActionReport | null>(null);

  const totalWasted = useMemo(() => sets.reduce((n, d) => n + d.reclaimable, 0), [sets]);

  const visible = useMemo(() => filterSort(sets, search, sortKey), [sets, search, sortKey]);

  // Drop sets that were fully actioned away after an action completes.
  const onActionDone = (report: ActionReport) => {
    setLastReport(report);
    const removed = new Set(report.items.filter((i) => i.ok).map((i) => i.source));
    useStore.setState((st) => ({
      sets: st.sets
        .map((d) => ({ ...d, members: d.members.filter((m) => !removed.has(m.entry.path)) }))
        .filter((d) => d.members.length > 1),
    }));
  };

  const doExport = async (format: "json" | "csv") => {
    const path = await api.pickSavePath(`duphunter-results.${format}`);
    if (path) await api.exportResults(sets, path, format);
  };

  // Keyboard shortcuts: Delete → recycle bin; 1/2/3/4 → bulk keep rules.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement)?.tagName === "INPUT") return;
      if (e.key === "Delete" && removalCount() > 0) setDialogKind("recycle-bin");
      else if (e.key === "1") applyBulk("newest");
      else if (e.key === "2") applyBulk("oldest");
      else if (e.key === "3") applyBulk("shortest-path");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [applyBulk, removalCount]);

  if (sets.length === 0) {
    return (
      <div className={s.empty}>
        <CheckmarkCircleRegular fontSize={48} style={{ color: tokens.colorPaletteGreenForeground1 }} />
        <Subtitle1>No duplicates found</Subtitle1>
        <Body1>
          Scanned {result?.filesScanned.toLocaleString() ?? 0} files
          {result?.cancelled ? " (cancelled)" : ""}. Nothing to clean up.
        </Body1>
        {result && result.issues.length > 0 && (
          <Caption1>{result.issues.length} files could not be read (permissions, etc.)</Caption1>
        )}
        <Button appearance="primary" icon={<ArrowLeftRegular />} onClick={() => setView("setup")}>
          New scan
        </Button>
      </div>
    );
  }

  return (
    <div className={s.layout}>
      <div className={s.main}>
        {/* Summary bar */}
        <div className={s.summary}>
          <Button appearance="subtle" icon={<ArrowLeftRegular />} onClick={() => setView("setup")}>
            New scan
          </Button>
          <ToolbarDivider />
          <div className={s.summaryItem}>
            <span className={s.summaryValue}>{sets.length}</span>
            <Caption1>duplicate sets</Caption1>
          </div>
          <div className={s.summaryItem}>
            <span className={s.summaryValue}>{formatBytes(totalWasted)}</span>
            <Caption1>total wasted</Caption1>
          </div>
          <div className={s.summaryItem}>
            <span className={s.summaryValue} style={{ color: tokens.colorPaletteGreenForeground1 }}>
              {formatBytes(reclaimableSelected())}
            </span>
            <Caption1>reclaimable by selection ({removalCount()} files)</Caption1>
          </div>
          {result?.issues.length ? (
            <Badge appearance="tint" color="warning">
              {result.issues.length} read errors
            </Badge>
          ) : null}
        </div>

        {lastReport && (
          <MessageBar intent="success">
            <MessageBarBody>
              {lastReport.kind} done — {lastReport.totalFiles} files, {formatBytes(lastReport.totalBytes)} freed.
              {lastReport.manifestPath ? ` Manifest: ${lastReport.manifestPath}` : ""}
            </MessageBarBody>
          </MessageBar>
        )}

        {/* Toolbar */}
        <Toolbar className={s.toolbar}>
          <Input
            contentBefore={<></>}
            placeholder="Search name or path…"
            value={search}
            onChange={(_, d) => setSearch(d.value)}
            style={{ minWidth: 220 }}
          />
          <ToolbarDivider />
          <Dropdown
            value={sortLabel(sortKey)}
            selectedOptions={[sortKey]}
            onOptionSelect={(_, d) => setSortKey(d.optionValue as SortKey)}
            style={{ minWidth: 170 }}
          >
            <Option value="reclaimable">Sort: reclaimable</Option>
            <Option value="size">Sort: file size</Option>
            <Option value="count">Sort: copies</Option>
            <Option value="path">Sort: path</Option>
          </Dropdown>
          <ToolbarDivider />
          <Caption1 style={{ alignSelf: "center" }}>Auto-keep:</Caption1>
          {([
            ["newest", "Newest"],
            ["oldest", "Oldest"],
            ["shortest-path", "Shortest path"],
          ] as [BulkRule, string][]).map(([rule, label]) => (
            <ToolbarButton key={rule} onClick={() => applyBulk(rule)}>
              {label}
            </ToolbarButton>
          ))}
          <div className={s.grow} />
          <ToolbarButton icon={<ArrowExportRegular />} onClick={() => doExport("json")}>
            JSON
          </ToolbarButton>
          <ToolbarButton icon={<ArrowExportRegular />} onClick={() => doExport("csv")}>
            CSV
          </ToolbarButton>
          <ToolbarDivider />
          <ToolbarButton icon={<ArchiveRegular />} onClick={() => setDialogKind("quarantine")}>
            Quarantine
          </ToolbarButton>
          <ToolbarButton icon={<FolderLinkRegular />} onClick={() => setDialogKind("hardlink")}>
            Hardlink
          </ToolbarButton>
          <ToolbarButton
            appearance="primary"
            icon={<DeleteRegular />}
            disabled={removalCount() === 0}
            onClick={() => setDialogKind("recycle-bin")}
          >
            Delete ({removalCount()})
          </ToolbarButton>
        </Toolbar>

        <VirtualList sets={visible} selected={selected} onSelect={setSelected} />
      </div>

      <PreviewPane file={selected} />

      {dialogKind && (
        <ActionDialog
          kind={dialogKind}
          open={!!dialogKind}
          onClose={() => setDialogKind(null)}
          onDone={onActionDone}
        />
      )}
    </div>
  );
}

function VirtualList({
  sets,
  selected,
  onSelect,
}: {
  sets: DupSet[];
  selected: FileEntry | null;
  onSelect: (f: FileEntry) => void;
}) {
  const s = useStyles();
  const parentRef = useRef<HTMLDivElement>(null);
  const virt = useVirtualizer({
    count: sets.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 150,
    overscan: 6,
  });

  return (
    <div className={s.list} ref={parentRef}>
      <div style={{ height: virt.getTotalSize(), position: "relative", width: "100%" }}>
        {virt.getVirtualItems().map((vi) => (
          <div
            key={sets[vi.index].id}
            data-index={vi.index}
            ref={virt.measureElement}
            style={{ position: "absolute", top: 0, left: 0, width: "100%", transform: `translateY(${vi.start}px)` }}
          >
            <SetCard set={sets[vi.index]} selectedPath={selected?.path} onSelect={onSelect} />
          </div>
        ))}
      </div>
    </div>
  );
}

function filterSort(sets: DupSet[], search: string, key: SortKey): DupSet[] {
  const q = search.trim().toLowerCase();
  let out = sets;
  if (q) {
    out = sets.filter((d) => d.members.some((m) => m.entry.path.toLowerCase().includes(q)));
  }
  const sorted = [...out];
  switch (key) {
    case "reclaimable":
      sorted.sort((a, b) => b.reclaimable - a.reclaimable);
      break;
    case "size":
      sorted.sort((a, b) => (b.members[0]?.entry.size ?? 0) - (a.members[0]?.entry.size ?? 0));
      break;
    case "count":
      sorted.sort((a, b) => b.members.length - a.members.length);
      break;
    case "path":
      sorted.sort((a, b) => (a.members[0]?.entry.path ?? "").localeCompare(b.members[0]?.entry.path ?? ""));
      break;
  }
  return sorted;
}

function sortLabel(k: SortKey): string {
  return { reclaimable: "Sort: reclaimable", size: "Sort: file size", count: "Sort: copies", path: "Sort: path" }[k];
}
