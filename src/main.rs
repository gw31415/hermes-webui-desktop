// Hermes WebUI Desktop — Tauri shell
// Spawns the Python WebUI server (bundled inside the app) as a child process,
// waits for /health, then the frontend loads the URL in an iframe.
// Kills the child on exit.

use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::Manager;

/// Holds the server URL so the frontend can query it via IPC.
struct ServerUrl(Mutex<String>);

/// Holds the child process handle so we can kill it on exit.
struct ServerChild(Mutex<Option<std::process::Child>>);

fn find_free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn wait_for_health(port: u16, timeout_secs: u64) -> bool {
    let url = format!("http://127.0.0.1:{}/health", port);
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    while std::time::Instant::now() < deadline {
        if let Ok(resp) = reqwest::blocking::get(&url) {
            if resp.status().is_success() {
                return true;
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
    false
}

/// Resolve the bundled hermes-webui directory.
///
/// In `cargo tauri dev` (debug): `<project_root>/hermes-webui/`
/// In a built `.app` (release): `<app_resource_dir>/hermes-webui/`
fn resolve_webui_dir(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    // 1) Try Tauri's resource directory (works in bundled mode)
    if let Some(resource_dir) = app.path().resource_dir().ok() {
        let candidate = resource_dir.join("hermes-webui");
        if candidate.join("bootstrap.py").exists() {
            return Some(candidate);
        }
    }

    // 2) Fallback: dev mode — look relative to CARGO_MANIFEST_DIR or executable
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let candidate = std::path::PathBuf::from(&manifest_dir).join("hermes-webui");
        if candidate.join("bootstrap.py").exists() {
            return Some(candidate);
        }
    }

    // 3) Fallback: relative to the executable's parent
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().skip(1) {
            let candidate = ancestor.join("hermes-webui");
            if candidate.join("bootstrap.py").exists() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Resolve the Python interpreter.
/// 1) Hermes agent venv (has all agent deps)
/// 2) System python3
fn resolve_python() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let hermes_py = format!("{}/.hermes/hermes-agent/venv/bin/python", home);
    if std::path::Path::new(&hermes_py).exists() {
        hermes_py
    } else {
        "python3".to_string()
    }
}

fn spawn_server(port: u16, webui_dir: &std::path::Path, py: &str) -> Option<std::process::Child> {
    // --foreground makes bootstrap.py os.execv() into server.py in the same
    // PID, so our Child handle owns the actual long-lived server process.
    // Without it, bootstrap.py spawns server.py detached and exits, leaving
    // our Child handle pointing at a dead process.
    let child = Command::new(py)
        .arg("bootstrap.py")
        .arg("--no-browser")
        .arg("--foreground")
        .current_dir(webui_dir)
        .env("HERMES_WEBUI_HOST", "127.0.0.1")
        .env("HERMES_WEBUI_PORT", port.to_string())
        .env("HERMES_WEBUI_SKIP_ONBOARDING", "1")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn();

    match child {
        Ok(c) => {
            eprintln!(
                "[hermes-desktop] Server spawned (pid={}, port={}, dir={})",
                c.id(),
                port,
                webui_dir.display()
            );
            Some(c)
        }
        Err(e) => {
            eprintln!("[hermes-desktop] Failed to spawn server: {}", e);
            None
        }
    }
}

#[tauri::command]
fn get_server_url(state: tauri::State<ServerUrl>) -> String {
    state.0.lock().unwrap().clone()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let port = find_free_port();
            let url = format!("http://127.0.0.1:{}", port);

            // Resolve the bundled WebUI directory
            let webui_dir =
                resolve_webui_dir(app.handle()).expect("[hermes-desktop] FATAL: hermes-webui directory not found in resources or project root");
            let py = resolve_python();

            eprintln!(
                "[hermes-desktop] WebUI dir: {}",
                webui_dir.display()
            );
            eprintln!("[hermes-desktop] Python: {}", py);

            // Spawn server
            let child = spawn_server(port, &webui_dir, &py);
            if child.is_some() {
                eprintln!("[hermes-desktop] Waiting for server health (60s timeout)...");
                if !wait_for_health(port, 60) {
                    eprintln!(
                        "[hermes-desktop] WARNING: Server not healthy within 60s — frontend will keep retrying"
                    );
                }
            }

            // Share state with frontend and window-event handler
            app.manage(ServerUrl(Mutex::new(url)));
            app.manage(ServerChild(Mutex::new(child)));

            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_server_url])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                eprintln!("[hermes-desktop] Window destroyed — killing server");
                if let Some(state) = window.app_handle().try_state::<ServerChild>() {
                    let mut guard = state.0.lock().unwrap();
                    if let Some(ref mut child) = *guard {
                        let _ = child.kill();
                        let _ = child.wait();
                        eprintln!("[hermes-desktop] Server killed");
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
