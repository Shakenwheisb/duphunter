import { useEffect, useMemo, useState } from "react";
import {
  FluentProvider,
  webDarkTheme,
  webLightTheme,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { useStore } from "./store";
import * as api from "./api";
import { ScanSetup } from "./screens/ScanSetup";
import { ScanningDashboard } from "./screens/ScanningDashboard";
import { Results } from "./screens/Results";
import { TitleBar } from "./components/TitleBar";

const useStyles = makeStyles({
  root: {
    height: "100vh",
    display: "flex",
    flexDirection: "column",
    backgroundColor: tokens.colorNeutralBackground2,
    color: tokens.colorNeutralForeground1,
  },
  body: { flex: 1, minHeight: 0, display: "flex", flexDirection: "column" },
});

/** Follow the OS dark/light setting and react to live changes. */
function useSystemTheme() {
  const [dark, setDark] = useState(
    () =>
      typeof window !== "undefined" &&
      window.matchMedia?.("(prefers-color-scheme: dark)").matches
  );
  useEffect(() => {
    const mq = window.matchMedia?.("(prefers-color-scheme: dark)");
    if (!mq) return;
    const handler = (e: MediaQueryListEvent) => setDark(e.matches);
    mq.addEventListener?.("change", handler);
    return () => mq.removeEventListener?.("change", handler);
  }, []);
  return dark;
}

export function App() {
  const styles = useStyles();
  const dark = useSystemTheme();
  const theme = useMemo(() => (dark ? webDarkTheme : webLightTheme), [dark]);

  const view = useStore((s) => s.view);
  const setProgress = useStore((s) => s.setProgress);
  const finishScan = useStore((s) => s.finishScan);
  const setError = useStore((s) => s.setError);
  const setFfmpeg = useStore((s) => s.setFfmpeg);

  // Wire backend events once, on mount.
  useEffect(() => {
    let unsubs: Array<() => void> = [];
    (async () => {
      setFfmpeg(await api.checkFfmpeg());
      unsubs.push(await api.onProgress(setProgress));
      unsubs.push(await api.onComplete(finishScan));
      unsubs.push(await api.onError((m) => setError(m)));
    })();
    return () => unsubs.forEach((u) => u());
  }, [setProgress, finishScan, setError, setFfmpeg]);

  return (
    <FluentProvider theme={theme} style={{ height: "100%" }}>
      <div className={styles.root}>
        <TitleBar />
        <div className={styles.body}>
          {view === "setup" && <ScanSetup />}
          {view === "scanning" && <ScanningDashboard />}
          {view === "results" && <Results />}
        </div>
      </div>
    </FluentProvider>
  );
}
