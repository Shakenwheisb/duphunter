import { useEffect, useState } from "react";
import { Button, Caption1, makeStyles, Text, tokens } from "@fluentui/react-components";
import { OpenRegular } from "@fluentui/react-icons";
import * as api from "../api";
import type { FileEntry } from "../types";
import { basename, formatBytes, formatDate, formatDuration, isAudioPath, isImagePath, isVideoPath } from "../util";

const useStyles = makeStyles({
  pane: {
    width: "340px",
    borderLeft: `1px solid ${tokens.colorNeutralStroke2}`,
    padding: "16px",
    display: "flex",
    flexDirection: "column",
    gap: "10px",
    overflowY: "auto",
    backgroundColor: tokens.colorNeutralBackground1,
  },
  media: {
    width: "100%",
    maxHeight: "240px",
    objectFit: "contain",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
  },
  placeholder: {
    height: "180px",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
    color: tokens.colorNeutralForeground3,
  },
  meta: { display: "flex", flexDirection: "column", gap: "3px" },
  metaRow: { display: "flex", justifyContent: "space-between", gap: "8px" },
});

export function PreviewPane({ file }: { file: FileEntry | null }) {
  const s = useStyles();
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setUrl(null);
    if (!file) return;
    (async () => {
      try {
        if (isVideoPath(file.path)) {
          // Use the source video directly for scrub/playback.
          const u = await api.assetUrl(file.path);
          if (!cancelled) setUrl(u);
        } else if (isImagePath(file.path) || isAudioPath(file.path)) {
          const u = await api.assetUrl(file.path);
          if (!cancelled) setUrl(u);
        }
      } catch {
        /* preview is best-effort */
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [file]);

  if (!file) {
    return (
      <div className={s.pane}>
        <div className={s.placeholder}>Select a file to preview</div>
      </div>
    );
  }

  const v = file.video;
  return (
    <div className={s.pane}>
      {url && isImagePath(file.path) && <img className={s.media} src={url} alt={basename(file.path)} />}
      {url && isVideoPath(file.path) && <video className={s.media} src={url} controls />}
      {url && isAudioPath(file.path) && <audio style={{ width: "100%" }} src={url} controls />}
      {!url && <div className={s.placeholder}>No inline preview</div>}

      <Text weight="semibold" style={{ wordBreak: "break-all" }}>
        {basename(file.path)}
      </Text>
      <Caption1 style={{ wordBreak: "break-all", opacity: 0.7 }}>{file.path}</Caption1>

      <div className={s.meta}>
        <MetaRow label="Size" value={formatBytes(file.size)} />
        <MetaRow label="Modified" value={formatDate(file.mtime)} />
        <MetaRow label="Root" value={basename(file.root)} />
        {v && (
          <>
            <MetaRow label="Duration" value={formatDuration(v.durationSecs)} />
            <MetaRow label="Resolution" value={v.width ? `${v.width}×${v.height}` : "—"} />
            <MetaRow label="Video codec" value={v.videoCodec || "—"} />
            <MetaRow label="Audio codec" value={v.audioCodec || "—"} />
            <MetaRow label="Bitrate" value={v.bitrate ? `${(v.bitrate / 1000).toFixed(0)} kbps` : "—"} />
          </>
        )}
      </div>

      <Button icon={<OpenRegular />} appearance="subtle" onClick={() => api.revealInExplorer(file.path)}>
        Reveal in file manager
      </Button>
    </div>
  );

  function MetaRow({ label, value }: { label: string; value: string }) {
    return (
      <div className={s.metaRow}>
        <Caption1 style={{ opacity: 0.6 }}>{label}</Caption1>
        <Caption1>{value}</Caption1>
      </div>
    );
  }
}
