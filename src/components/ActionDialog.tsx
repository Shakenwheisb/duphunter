import { useEffect, useState } from "react";
import {
  Button,
  Caption1,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  MessageBar,
  MessageBarBody,
  Spinner,
  Switch,
  Text,
  tokens,
} from "@fluentui/react-components";
import { DeleteRegular, WarningRegular } from "@fluentui/react-icons";
import * as api from "../api";
import { useStore } from "../store";
import type { ActionKind, ActionPlan, ActionReport } from "../types";
import { basename, formatBytes } from "../util";

const LABELS: Record<ActionKind, string> = {
  "recycle-bin": "Move to Recycle Bin",
  "permanent-delete": "Permanently delete",
  quarantine: "Move to quarantine folder",
  hardlink: "Replace with hardlinks",
  symlink: "Replace with symlinks",
};

export function ActionDialog({
  kind,
  open,
  onClose,
  onDone,
}: {
  kind: ActionKind;
  open: boolean;
  onClose: () => void;
  onDone: (report: ActionReport) => void;
}) {
  const removalTargets = useStore((s) => s.removalTargets);
  const reclaimableSelected = useStore((s) => s.reclaimableSelected);

  const [allowPermanent, setAllowPermanent] = useState(false);
  const [quarantineDir, setQuarantineDir] = useState<string | null>(null);
  const [preview, setPreview] = useState<ActionReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const targets = removalTargets();
  const fileCount = targets.reduce((n, t) => n + t.remove.length, 0);
  const isPermanent = kind === "permanent-delete";
  const needsQuarantine = kind === "quarantine";

  const buildPlan = (dryRun: boolean): ActionPlan => ({
    kind,
    dryRun,
    quarantineDir,
    targets,
  });

  // Compute a dry-run preview whenever the dialog opens.
  useEffect(() => {
    if (!open) {
      setPreview(null);
      setError(null);
      return;
    }
    (async () => {
      try {
        setPreview(await api.runAction(buildPlan(true)));
      } catch (e) {
        setError(String(e));
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, quarantineDir]);

  const chooseQuarantine = async () => {
    const dir = await api.pickFolder();
    if (dir) setQuarantineDir(dir);
  };

  const confirm = async () => {
    setBusy(true);
    setError(null);
    try {
      const report = await api.runAction(buildPlan(false));
      onDone(report);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const blocked =
    fileCount === 0 ||
    (isPermanent && !allowPermanent) ||
    (needsQuarantine && !quarantineDir);

  return (
    <Dialog open={open} onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{LABELS[kind]}</DialogTitle>
          <DialogContent>
            <Text>
              This will affect <b>{fileCount}</b> file{fileCount === 1 ? "" : "s"} across{" "}
              <b>{targets.length}</b> set{targets.length === 1 ? "" : "s"}, freeing about{" "}
              <b style={{ color: tokens.colorPaletteGreenForeground1 }}>
                {formatBytes(reclaimableSelected())}
              </b>
              . At least one copy is always kept in every set.
            </Text>

            {isPermanent && (
              <MessageBar intent="error" style={{ marginTop: 12 }}>
                <MessageBarBody>
                  <WarningRegular /> Permanent deletion cannot be undone. Files will NOT go to the
                  Recycle Bin.
                </MessageBarBody>
              </MessageBar>
            )}
            {isPermanent && (
              <Switch
                style={{ marginTop: 8 }}
                checked={allowPermanent}
                onChange={(_, d) => setAllowPermanent(d.checked)}
                label="I understand — delete permanently"
              />
            )}

            {needsQuarantine && (
              <div style={{ marginTop: 12 }}>
                <Button onClick={chooseQuarantine}>
                  {quarantineDir ? `Quarantine: ${basename(quarantineDir)}` : "Choose quarantine folder…"}
                </Button>
              </div>
            )}

            <Text weight="semibold" style={{ display: "block", marginTop: 16 }}>
              Dry-run preview
            </Text>
            {!preview && !error && <Spinner size="tiny" label="Computing preview…" />}
            {error && (
              <MessageBar intent="error">
                <MessageBarBody>{error}</MessageBarBody>
              </MessageBar>
            )}
            {preview && (
              <div
                style={{
                  maxHeight: 200,
                  overflowY: "auto",
                  marginTop: 6,
                  fontFamily: "ui-monospace, Menlo, Consolas, monospace",
                  fontSize: 12,
                }}
              >
                {preview.items.slice(0, 200).map((it, i) => (
                  <Caption1 key={i} style={{ display: "block", wordBreak: "break-all", opacity: 0.8 }}>
                    {it.ok ? "✓" : "✗"} {it.source}
                  </Caption1>
                ))}
                {preview.items.length > 200 && (
                  <Caption1>…and {preview.items.length - 200} more</Caption1>
                )}
              </div>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button
              appearance="primary"
              icon={busy ? <Spinner size="tiny" /> : <DeleteRegular />}
              disabled={blocked || busy}
              onClick={confirm}
            >
              {LABELS[kind]}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
