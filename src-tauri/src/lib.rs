//! Tauri backend for DupHunter.
//!
//! This layer is deliberately thin: it owns the window/IPC and a single shared
//! [`ScanControl`], and delegates all real work to `dupcore`. Scans run on a
//! background thread so the UI never blocks; progress is pushed to the frontend
//! as throttled `scan-progress` events, with a final `scan-complete` event.

use dupcore::control::{ProgressSink, ScanControl};
use dupcore::model::{ActionPlan, ActionReport, Progress, ScanConfig, ScanResult};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

/// Shared scan control so pause/resume/cancel commands can reach a running scan.
#[derive(Default)]
struct AppState {
    control: Mutex<Option<ScanControl>>,
}

/// Progress sink that emits throttled IPC events to the frontend.
struct EmitSink {
    app: AppHandle,
    /// Last emit time in millis since epoch; used to cap event rate (~20/sec).
    last_ms: AtomicU64,
}

impl EmitSink {
    fn new(app: AppHandle) -> Self {
        Self { app, last_ms: AtomicU64::new(0) }
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl ProgressSink for EmitSink {
    fn report(&self, progress: &Progress) {
        // Always emit terminal phases; throttle the high-frequency ones.
        let terminal = matches!(progress.phase, dupcore::model::Phase::Done);
        let now = now_ms();
        let last = self.last_ms.load(Ordering::Relaxed);
        if terminal || now.saturating_sub(last) >= 50 {
            self.last_ms.store(now, Ordering::Relaxed);
            let _ = self.app.emit("scan-progress", progress);
        }
    }
}

/// Final payload for the `scan-complete` event.
#[derive(Serialize, Clone)]
struct ScanComplete {
    result: ScanResult,
}

#[tauri::command]
fn check_ffmpeg() -> bool {
    dupcore::video::ffmpeg_available()
}

/// Kick off a scan on a background thread. Returns immediately; results arrive
/// via the `scan-complete` event.
#[tauri::command]
fn start_scan(app: AppHandle, state: State<'_, AppState>, config: ScanConfig) -> Result<(), String> {
    let control = ScanControl::new();
    {
        let mut guard = state.control.lock().unwrap();
        if guard.is_some() {
            return Err("a scan is already running".into());
        }
        *guard = Some(control.clone());
    }

    let app_for_thread = app.clone();
    std::thread::spawn(move || {
        let sink = EmitSink::new(app_for_thread.clone());
        let result = dupcore::scan(&config, &control, &sink);

        // Clear the shared control so a new scan can start.
        if let Some(state) = app_for_thread.try_state::<AppState>() {
            *state.control.lock().unwrap() = None;
        }

        match result {
            Ok(result) => {
                let _ = app_for_thread.emit("scan-complete", ScanComplete { result });
            }
            Err(e) => {
                let _ = app_for_thread.emit("scan-error", e.to_string());
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn pause_scan(state: State<'_, AppState>) {
    if let Some(c) = state.control.lock().unwrap().as_ref() {
        c.pause();
    }
}

#[tauri::command]
fn resume_scan(state: State<'_, AppState>) {
    if let Some(c) = state.control.lock().unwrap().as_ref() {
        c.resume();
    }
}

#[tauri::command]
fn cancel_scan(state: State<'_, AppState>) {
    if let Some(c) = state.control.lock().unwrap().as_ref() {
        c.cancel();
    }
}

/// Run (or dry-run) a destructive action. Manifests are written under app data.
#[tauri::command]
fn run_action(app: AppHandle, plan: ActionPlan) -> Result<ActionReport, String> {
    let manifest_dir = app
        .path()
        .app_data_dir()
        .map(|d| d.join("manifests"))
        .unwrap_or_else(|_| PathBuf::from("."));
    dupcore::actions::execute(&plan, &manifest_dir).map_err(|e| e.to_string())
}

/// Export results to JSON or CSV. `format` is "json" or "csv".
#[tauri::command]
fn export_results(
    sets: Vec<dupcore::model::DupSet>,
    path: PathBuf,
    format: String,
) -> Result<(), String> {
    match format.as_str() {
        "json" => dupcore::export::to_json(&sets, &path).map_err(|e| e.to_string()),
        "csv" => dupcore::export::to_csv(&sets, &path).map_err(|e| e.to_string()),
        other => Err(format!("unknown export format: {other}")),
    }
}

/// Extract a poster thumbnail (JPEG) for a video into the app cache, returning
/// its path so the frontend can show it via `convertFileSrc`.
#[tauri::command]
fn video_thumbnail(app: AppHandle, path: PathBuf) -> Result<PathBuf, String> {
    let cache = app
        .path()
        .app_cache_dir()
        .map(|d| d.join("thumbs"))
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    // Cache key from the absolute path hash.
    let key = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        path.hash(&mut h);
        format!("{:016x}.jpg", h.finish())
    };
    let out = cache.join(key);
    if out.exists() {
        return Ok(out);
    }
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-ss", "3", "-i"])
        .arg(&path)
        .args(["-frames:v", "1", "-vf", "scale=320:-1", "-y"])
        .arg(&out)
        .status()
        .map_err(|_| "ffmpeg not found".to_string())?;
    if status.success() && out.exists() {
        Ok(out)
    } else {
        Err("thumbnail extraction failed".into())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            check_ffmpeg,
            start_scan,
            pause_scan,
            resume_scan,
            cancel_scan,
            run_action,
            export_results,
            video_thumbnail
        ])
        .run(tauri::generate_context!())
        .expect("error while running DupHunter");
}
