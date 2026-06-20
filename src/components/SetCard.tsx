import { useEffect, useState } from "react";
import {
  Badge,
  Button,
  Caption1,
  Card,
  makeStyles,
  mergeClasses,
  Radio,
  Text,
  tokens,
} from "@fluentui/react-components";
import { DocumentRegular, LinkRegular, VideoClipRegular } from "@fluentui/react-icons";
import { useStore } from "../store";
import * as api from "../api";
import type { DupMember, DupSet, FileEntry } from "../types";
import { basename, formatBytes, formatDate, formatDuration, isImagePath, isVideoPath } from "../util";

const useStyles = makeStyles({
  card: { padding: "12px", marginBottom: "10px" },
  header: { display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "8px" },
  headLeft: { display: "flex", alignItems: "center", gap: "10px" },
  members: { display: "flex", flexDirection: "column", gap: "4px" },
  row: {
    display: "flex",
    alignItems: "center",
    gap: "10px",
    padding: "6px 8px",
    borderRadius: tokens.borderRadiusMedium,
    cursor: "pointer",
    ":hover": { backgroundColor: tokens.colorNeutralBackground2Hover },
  },
  rowSelected: { backgroundColor: tokens.colorBrandBackground2 },
  thumb: {
    width: "56px",
    height: "40px",
    objectFit: "cover",
    borderRadius: tokens.borderRadiusSmall,
    backgroundColor: tokens.colorNeutralBackground3,
    flexShrink: 0,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    color: tokens.colorNeutralForeground3,
  },
  info: { flex: 1, minWidth: 0 },
  path: { whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", opacity: 0.65 },
  removed: { opacity: 0.5, textDecoration: "line-through" },
});

export function SetCard({
  set,
  selectedPath,
  onSelect,
}: {
  set: DupSet;
  selectedPath?: string;
  onSelect: (f: FileEntry) => void;
}) {
  const s = useStyles();
  const setKeeper = useStore((st) => st.setKeeper);
  const toggleRemove = useStore((st) => st.toggleRemove);
  const keepAll = useStore((st) => st.keepAll);

  return (
    <Card className={s.card}>
      <div className={s.header}>
        <div className={s.headLeft}>
          {set.mode === "video-near-dup" ? <VideoClipRegular /> : <DocumentRegular />}
          <Text weight="semibold">
            {set.members.length} copies · {formatBytes(set.members[0]?.entry.size ?? 0)} each
          </Text>
          {set.zeroByte && <Badge color="informative">zero-byte</Badge>}
          {set.similarity != null && (
            <Badge appearance="tint" color="brand">
              {set.similarity}% similar
            </Badge>
          )}
          <Badge appearance="tint" color="success">
            reclaim {formatBytes(set.reclaimable)}
          </Badge>
        </div>
        <Button size="small" appearance="subtle" onClick={() => keepAll(set.id)}>
          Keep all
        </Button>
      </div>

      <div className={s.members}>
        {set.members.map((m) => (
          <MemberRow
            key={m.entry.path}
            member={m}
            setId={set.id}
            selected={selectedPath === m.entry.path}
            onSelect={() => onSelect(m.entry)}
            onKeep={() => setKeeper(set.id, m.entry.path)}
            onToggle={() => toggleRemove(set.id, m.entry.path)}
            disabledRemove={set.zeroByte}
          />
        ))}
      </div>
    </Card>
  );
}

function MemberRow({
  member,
  selected,
  onSelect,
  onKeep,
  onToggle,
  disabledRemove,
}: {
  member: DupMember;
  setId: number;
  selected: boolean;
  onSelect: () => void;
  onKeep: () => void;
  onToggle: () => void;
  disabledRemove: boolean;
}) {
  const s = useStyles();
  const e = member.entry;
  const removed = member.role === "remove";

  return (
    <div className={mergeClasses(s.row, selected && s.rowSelected)} onClick={onSelect}>
      <Radio
        checked={member.role === "keep"}
        onClick={(ev) => {
          ev.stopPropagation();
          onKeep();
        }}
        label=""
        aria-label="Keep this copy"
      />
      <Thumb file={e} />
      <div className={s.info}>
        <Text className={removed ? s.removed : undefined} weight={member.role === "keep" ? "semibold" : "regular"}>
          {basename(e.path)}
        </Text>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <Caption1 className={s.path}>{e.path}</Caption1>
        </div>
        <div style={{ display: "flex", gap: 8, marginTop: 2 }}>
          <Badge size="small" appearance="outline">
            {basename(e.root)}
          </Badge>
          <Caption1>{formatBytes(e.size)}</Caption1>
          <Caption1>· {formatDate(e.mtime)}</Caption1>
          {e.video && <Caption1>· {formatDuration(e.video.durationSecs)}</Caption1>}
          {e.video?.width ? <Caption1>· {e.video.width}×{e.video.height}</Caption1> : null}
          {member.isHardlinkOfOther && (
            <Badge size="small" color="informative" icon={<LinkRegular />}>
              hardlink — frees no space
            </Badge>
          )}
        </div>
      </div>
      <Button
        size="small"
        appearance={removed ? "primary" : "outline"}
        disabled={disabledRemove}
        onClick={(ev) => {
          ev.stopPropagation();
          onToggle();
        }}
      >
        {removed ? "Will remove" : "Keep"}
      </Button>
    </div>
  );
}

/** Lazy thumbnail: image files load directly; videos request a poster frame. */
function Thumb({ file }: { file: FileEntry }) {
  const s = useStyles();
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        if (isImagePath(file.path)) {
          const u = await api.assetUrl(file.path);
          if (!cancelled) setSrc(u);
        } else if (isVideoPath(file.path) && api.inTauri()) {
          const thumb = await api.videoThumbnail(file.path);
          const u = await api.assetUrl(thumb);
          if (!cancelled) setSrc(u);
        }
      } catch {
        /* fall back to icon */
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [file.path]);

  if (src) return <img className={s.thumb} src={src} alt="" />;
  return (
    <div className={s.thumb}>
      {isVideoPath(file.path) ? <VideoClipRegular /> : <DocumentRegular />}
    </div>
  );
}
