import { useEffect, useState } from "react";
import {
  Body1,
  Button,
  Card,
  Caption1,
  Dropdown,
  Field,
  Input,
  makeStyles,
  mergeClasses,
  Option,
  Radio,
  RadioGroup,
  Slider,
  Subtitle2,
  Switch,
  Text,
  Tag,
  TagGroup,
  tokens,
  Tooltip,
} from "@fluentui/react-components";
import {
  AddRegular,
  DeleteRegular,
  FolderAddRegular,
  PlayRegular,
  WarningRegular,
} from "@fluentui/react-icons";
import { useStore } from "../store";
import * as api from "../api";
import { basename } from "../util";

const useStyles = makeStyles({
  scroll: { flex: 1, overflowY: "auto", padding: "20px 24px" },
  grid: {
    display: "grid",
    gridTemplateColumns: "1fr 1fr",
    gap: "16px",
    maxWidth: "1100px",
    margin: "0 auto",
  },
  full: { gridColumn: "1 / -1" },
  card: { padding: "16px", display: "flex", flexDirection: "column", gap: "12px" },
  dropZone: {
    border: `2px dashed ${tokens.colorNeutralStroke2}`,
    borderRadius: tokens.borderRadiusXLarge,
    padding: "22px",
    textAlign: "center",
    transition: "all 120ms ease",
  },
  dropActive: {
    border: `2px dashed ${tokens.colorBrandStroke1}`,
    backgroundColor: tokens.colorBrandBackground2,
  },
  rootRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    padding: "8px 10px",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
  },
  rootList: { display: "flex", flexDirection: "column", gap: "6px", marginTop: "8px" },
  row2: { display: "flex", gap: "12px", flexWrap: "wrap" },
  footer: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    padding: "14px 24px",
    borderTop: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: tokens.colorNeutralBackground1,
  },
  chips: { marginTop: "6px" },
});

export function ScanSetup() {
  const s = useStyles();
  const { config, patchConfig, setMode, addRoots, removeRoot, recents } = useStore();
  const ffmpeg = useStore((st) => st.ffmpegAvailable);
  const beginScan = useStore((st) => st.beginScan);
  const error = useStore((st) => st.error);
  const setError = useStore((st) => st.setError);

  const [dragging, setDragging] = useState(false);
  const [extInput, setExtInput] = useState("");
  const [globInput, setGlobInput] = useState("");

  // Native drag-and-drop of folders (Tauri only).
  useEffect(() => {
    if (!api.inTauri()) return;
    let un: (() => void) | undefined;
    (async () => {
      const { getCurrentWebview } = await import("@tauri-apps/api/webview");
      un = await getCurrentWebview().onDragDropEvent((e) => {
        if (e.payload.type === "over") setDragging(true);
        else if (e.payload.type === "drop") {
          setDragging(false);
          addRoots(e.payload.paths);
        } else setDragging(false);
      });
    })();
    return () => un?.();
  }, [addRoots]);

  const pick = async () => {
    const picked = await api.pickFolders();
    if (picked.length) addRoots(picked);
  };

  const videoMode = config.mode === "video-near-dup";

  const start = async () => {
    setError(null);
    if (config.roots.length === 0) {
      setError("Add at least one folder to scan.");
      return;
    }
    if (videoMode && !ffmpeg) {
      setError(
        "Video mode needs ffmpeg/ffprobe on your PATH. Install FFmpeg (e.g. `winget install Gyan.FFmpeg`) and restart DupHunter."
      );
      return;
    }
    beginScan();
    try {
      await api.startScan(config);
    } catch (e) {
      setError(String(e));
    }
  };

  const addExt = () => {
    const v = extInput.trim().replace(/^\./, "");
    if (v) {
      patchConfig({
        excludes: {
          ...config.excludes,
          excludeExtensions: Array.from(new Set([...config.excludes.excludeExtensions, v])),
        },
      });
      setExtInput("");
    }
  };
  const addGlob = () => {
    const v = globInput.trim();
    if (v) {
      patchConfig({
        excludes: {
          ...config.excludes,
          globPatterns: Array.from(new Set([...config.excludes.globPatterns, v])),
        },
      });
      setGlobInput("");
    }
  };

  return (
    <>
      <div className={mergeClasses(s.scroll, "screen-enter")}>
        <div className={s.grid}>
          {/* Folders */}
          <Card className={mergeClasses(s.card, s.full)}>
            <Subtitle2>Folders to scan</Subtitle2>
            <Caption1>
              Every file across all added folders is compared against every other — duplicates are
              found within and across folders.
            </Caption1>
            <div
              className={mergeClasses(s.dropZone, dragging && s.dropActive)}
              onClick={pick}
              style={{ cursor: "pointer" }}
            >
              <FolderAddRegular fontSize={30} />
              <Body1 style={{ display: "block", marginTop: 8 }}>
                Drag folders here, or click to browse
              </Body1>
            </div>

            {config.roots.length > 0 && (
              <div className={s.rootList}>
                {config.roots.map((r) => (
                  <div key={r} className={s.rootRow}>
                    <div>
                      <Text weight="semibold">{basename(r)}</Text>
                      <br />
                      <Caption1 style={{ opacity: 0.65 }}>{r}</Caption1>
                    </div>
                    <Button
                      appearance="subtle"
                      icon={<DeleteRegular />}
                      onClick={() => removeRoot(r)}
                      aria-label={`Remove ${r}`}
                    />
                  </div>
                ))}
              </div>
            )}

            {recents.length > 0 && (
              <div>
                <Caption1>Recent locations</Caption1>
                <div className={s.chips}>
                  <TagGroup onDismiss={() => {}}>
                    {recents
                      .filter((r) => !config.roots.includes(r))
                      .map((r) => (
                        <Tag
                          key={r}
                          icon={<AddRegular />}
                          shape="circular"
                          style={{ cursor: "pointer", marginRight: 6 }}
                          onClick={() => addRoots([r])}
                        >
                          {basename(r)}
                        </Tag>
                      ))}
                  </TagGroup>
                </div>
              </div>
            )}
          </Card>

          {/* Mode */}
          <Card className={s.card}>
            <Subtitle2>Detection mode</Subtitle2>
            <RadioGroup
              value={config.mode}
              onChange={(_, d) => setMode(d.value as typeof config.mode)}
            >
              <Radio value="exact" label="Exact duplicates (byte-identical)" />
              <Radio
                value="video-near-dup"
                label="Video near-duplicates (same content, different format/length)"
              />
            </RadioGroup>
            {videoMode && !ffmpeg && (
              <Text style={{ color: tokens.colorPaletteRedForeground1 }}>
                <WarningRegular /> ffmpeg/ffprobe not detected — install FFmpeg to use this mode.
              </Text>
            )}
            {videoMode && (
              <>
                <Field label={`Similarity threshold: ${config.video.similarityThreshold}%`}>
                  <Slider
                    min={60}
                    max={100}
                    value={config.video.similarityThreshold}
                    onChange={(_, d) =>
                      patchConfig({ video: { ...config.video, similarityThreshold: d.value } })
                    }
                  />
                </Field>
                <Field label={`Frames sampled per video: ${config.video.frameSamples}`}>
                  <Slider
                    min={3}
                    max={11}
                    value={config.video.frameSamples}
                    onChange={(_, d) =>
                      patchConfig({ video: { ...config.video, frameSamples: d.value } })
                    }
                  />
                </Field>
              </>
            )}
          </Card>

          {/* Hash & safety options */}
          <Card className={s.card}>
            <Subtitle2>Comparison options</Subtitle2>
            <Field label="Hash algorithm" hint="Used for exact matching.">
              <Dropdown
                disabled={videoMode}
                value={hashLabel(config.hashAlgo)}
                selectedOptions={[config.hashAlgo]}
                onOptionSelect={(_, d) =>
                  patchConfig({ hashAlgo: d.optionValue as typeof config.hashAlgo })
                }
              >
                <Option value="blake3">BLAKE3 (fast, default)</Option>
                <Option value="xxh3">xxHash3 (fastest)</Option>
                <Option value="sha256">SHA-256 (cryptographically strong)</Option>
              </Dropdown>
            </Field>
            <Tooltip
              content="Stream a byte-for-byte comparison on hash-identical files to rule out the rare collision before deletion."
              relationship="description"
            >
              <Switch
                disabled={videoMode}
                checked={config.paranoid}
                onChange={(_, d) => patchConfig({ paranoid: d.checked })}
                label="Paranoid byte-for-byte verification"
              />
            </Tooltip>
            <Field label="Symbolic links">
              <RadioGroup
                layout="horizontal"
                value={config.symlinks}
                onChange={(_, d) => patchConfig({ symlinks: d.value as typeof config.symlinks })}
              >
                <Radio value="skip" label="Skip" />
                <Radio value="follow" label="Follow" />
              </RadioGroup>
            </Field>
          </Card>

          {/* Exclude rules */}
          <Card className={mergeClasses(s.card, s.full)}>
            <Subtitle2>Exclude rules</Subtitle2>
            <div className={s.row2}>
              <Field label="Min size (MB)">
                <Input
                  type="number"
                  value={config.excludes.minSize ? String(config.excludes.minSize / 1e6) : ""}
                  onChange={(_, d) =>
                    patchConfig({
                      excludes: {
                        ...config.excludes,
                        minSize: d.value ? Math.round(Number(d.value) * 1e6) : null,
                      },
                    })
                  }
                />
              </Field>
              <Field label="Max size (MB)">
                <Input
                  type="number"
                  value={config.excludes.maxSize ? String(config.excludes.maxSize / 1e6) : ""}
                  onChange={(_, d) =>
                    patchConfig({
                      excludes: {
                        ...config.excludes,
                        maxSize: d.value ? Math.round(Number(d.value) * 1e6) : null,
                      },
                    })
                  }
                />
              </Field>
              <Field label="Exclude extension" style={{ minWidth: 200 }}>
                <div style={{ display: "flex", gap: 6 }}>
                  <Input
                    placeholder="e.g. tmp"
                    value={extInput}
                    onChange={(_, d) => setExtInput(d.value)}
                    onKeyDown={(e) => e.key === "Enter" && addExt()}
                  />
                  <Button icon={<AddRegular />} onClick={addExt} />
                </div>
              </Field>
              <Field label="Exclude glob" style={{ minWidth: 240 }}>
                <div style={{ display: "flex", gap: 6 }}>
                  <Input
                    placeholder="**/node_modules/**"
                    value={globInput}
                    onChange={(_, d) => setGlobInput(d.value)}
                    onKeyDown={(e) => e.key === "Enter" && addGlob()}
                  />
                  <Button icon={<AddRegular />} onClick={addGlob} />
                </div>
              </Field>
            </div>
            <Switch
              checked={config.excludes.skipHiddenSystem}
              onChange={(_, d) =>
                patchConfig({ excludes: { ...config.excludes, skipHiddenSystem: d.checked } })
              }
              label="Skip hidden and system files"
            />
            <TagGroup
              onDismiss={(_, d) => {
                const id = String(d.value);
                patchConfig({
                  excludes: {
                    ...config.excludes,
                    excludeExtensions: config.excludes.excludeExtensions.filter(
                      (x) => `ext:${x}` !== id
                    ),
                    globPatterns: config.excludes.globPatterns.filter((x) => `glob:${x}` !== id),
                  },
                });
              }}
            >
              {config.excludes.excludeExtensions.map((x) => (
                <Tag key={`ext:${x}`} value={`ext:${x}`} dismissible>
                  .{x}
                </Tag>
              ))}
              {config.excludes.globPatterns.map((x) => (
                <Tag key={`glob:${x}`} value={`glob:${x}`} dismissible>
                  {x}
                </Tag>
              ))}
            </TagGroup>
          </Card>
        </div>
      </div>

      <div className={s.footer}>
        <div>
          <Text weight="semibold">
            {config.roots.length} folder{config.roots.length === 1 ? "" : "s"} ·{" "}
            {config.mode === "exact" ? "Exact" : "Video near-dup"} mode
          </Text>
          {error && (
            <Text style={{ color: tokens.colorPaletteRedForeground1, display: "block" }}>
              {error}
            </Text>
          )}
        </div>
        <Button
          appearance="primary"
          size="large"
          icon={<PlayRegular />}
          disabled={config.roots.length === 0}
          onClick={start}
        >
          Start scan
        </Button>
      </div>
    </>
  );
}

function hashLabel(a: string): string {
  return a === "blake3" ? "BLAKE3 (fast, default)" : a === "xxh3" ? "xxHash3 (fastest)" : "SHA-256";
}
