import { useState } from "react";
import {
  Badge,
  Body1,
  Button,
  Card,
  Caption1,
  Divider,
  makeStyles,
  mergeClasses,
  ProgressBar,
  Subtitle1,
  Text,
  tokens,
} from "@fluentui/react-components";
import {
  CheckmarkCircleRegular,
  CircleRegular,
  ArrowSyncRegular,
  DismissRegular,
  PauseRegular,
  PlayRegular,
} from "@fluentui/react-icons";
import { useStore } from "../store";
import * as api from "../api";
import { PHASE_LABELS, PHASE_ORDER_EXACT, PHASE_ORDER_VIDEO, formatBytes, formatDuration } from "../util";
import type { Phase } from "../types";

const useStyles = makeStyles({
  wrap: { flex: 1, overflowY: "auto", padding: "24px", display: "flex", justifyContent: "center" },
  inner: { width: "100%", maxWidth: "860px", display: "flex", flexDirection: "column", gap: "16px" },
  statGrid: { display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: "12px" },
  stat: { padding: "14px", textAlign: "center" },
  statValue: { fontSize: "22px", fontWeight: 700, lineHeight: 1.1 },
  phases: { display: "flex", flexDirection: "column", gap: "8px", padding: "16px" },
  phaseRow: { display: "flex", alignItems: "center", gap: "10px" },
  current: {
    fontFamily: "ui-monospace, Menlo, Consolas, monospace",
    fontSize: "12px",
    opacity: 0.8,
    wordBreak: "break-all",
  },
  controls: { display: "flex", gap: "10px", justifyContent: "center" },
});

export function ScanningDashboard() {
  const s = useStyles();
  const progress = useStore((st) => st.progress);
  const config = useStore((st) => st.config);
  const setView = useStore((st) => st.setView);
  const [paused, setPaused] = useState(false);

  const order = config.mode === "exact" ? PHASE_ORDER_EXACT : PHASE_ORDER_VIDEO;
  const phase = progress?.phase ?? "discovering";
  const filesDone = progress?.filesDone ?? 0;
  const filesTotal = progress?.filesTotal ?? 0;
  const elapsed = progress?.elapsedSecs ?? 0;

  const pct = filesTotal > 0 ? Math.min(1, filesDone / filesTotal) : undefined;
  const filesPerSec = elapsed > 0 ? filesDone / elapsed : 0;
  const mbPerSec = elapsed > 0 ? (progress?.bytesDone ?? 0) / 1e6 / elapsed : 0;
  const remaining = filesTotal - filesDone;
  const eta = filesPerSec > 0 && remaining > 0 ? remaining / filesPerSec : 0;

  const togglePause = async () => {
    if (paused) {
      await api.resumeScan();
      setPaused(false);
    } else {
      await api.pauseScan();
      setPaused(true);
    }
  };
  const cancel = async () => {
    await api.cancelScan();
    setView("setup");
  };

  return (
    <div className={mergeClasses(s.wrap, "screen-enter")}>
      <div className={s.inner}>
        <Card style={{ padding: 20 }}>
          <Subtitle1>
            {paused ? "Paused" : "Scanning"} — {PHASE_LABELS[phase]}
          </Subtitle1>
          <div style={{ margin: "14px 0" }}>
            <ProgressBar value={pct} thickness="large" />
          </div>
          <Text>
            {filesDone.toLocaleString()} of {filesTotal ? `~${filesTotal.toLocaleString()}` : "…"}{" "}
            files
          </Text>
          <Caption1 className={s.current} style={{ display: "block", marginTop: 6 }}>
            {progress?.currentPath ?? "Preparing…"}
          </Caption1>
        </Card>

        <div className={s.statGrid}>
          <Card className={s.stat}>
            <div className={s.statValue}>{filesPerSec.toFixed(0)}</div>
            <Caption1>files / sec</Caption1>
          </Card>
          <Card className={s.stat}>
            <div className={s.statValue}>{mbPerSec.toFixed(1)}</div>
            <Caption1>MB / sec</Caption1>
          </Card>
          <Card className={s.stat}>
            <div className={s.statValue}>{formatDuration(elapsed)}</div>
            <Caption1>elapsed</Caption1>
          </Card>
          <Card className={s.stat}>
            <div className={s.statValue}>{eta > 0 ? formatDuration(eta) : "—"}</div>
            <Caption1>ETA</Caption1>
          </Card>
        </div>

        <Card style={{ padding: 16 }}>
          <div style={{ display: "flex", justifyContent: "space-between" }}>
            <div>
              <Caption1>Duplicate sets found</Caption1>
              <div className={s.statValue}>{progress?.dupSets ?? 0}</div>
            </div>
            <div style={{ textAlign: "right" }}>
              <Caption1>Reclaimable so far</Caption1>
              <div className={s.statValue} style={{ color: tokens.colorPaletteGreenForeground1 }}>
                {formatBytes(progress?.reclaimable ?? 0)}
              </div>
            </div>
          </div>
        </Card>

        <Card className={s.phases}>
          {order.map((p) => (
            <PhaseItem key={p} phase={p} current={phase} order={order} className={s.phaseRow} />
          ))}
        </Card>

        <Divider />
        <div className={s.controls}>
          <Button
            appearance="secondary"
            icon={paused ? <PlayRegular /> : <PauseRegular />}
            onClick={togglePause}
          >
            {paused ? "Resume" : "Pause"}
          </Button>
          <Button appearance="outline" icon={<DismissRegular />} onClick={cancel}>
            Cancel
          </Button>
        </div>
      </div>
    </div>
  );
}

function PhaseItem({
  phase,
  current,
  order,
  className,
}: {
  phase: Phase;
  current: Phase;
  order: Phase[];
  className: string;
}) {
  const curIdx = current === "done" ? order.length : order.indexOf(current);
  const myIdx = order.indexOf(phase);
  const done = myIdx < curIdx;
  const active = myIdx === curIdx;
  return (
    <div className={className}>
      {done ? (
        <CheckmarkCircleRegular style={{ color: tokens.colorPaletteGreenForeground1 }} />
      ) : active ? (
        <ArrowSyncRegular
          style={{ color: tokens.colorBrandForeground1, animation: "spin 1s linear infinite" }}
        />
      ) : (
        <CircleRegular style={{ opacity: 0.4 }} />
      )}
      <Body1 style={{ opacity: done || active ? 1 : 0.5, fontWeight: active ? 600 : 400 }}>
        {PHASE_LABELS[phase]}
      </Body1>
      {active && (
        <Badge size="small" appearance="tint">
          in progress
        </Badge>
      )}
    </div>
  );
}
