#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod context;
mod ipc_server;
mod register;
mod schema_manager;
mod tray;
mod ui;
mod ximed_server;

use crate::context::SharedInputContext;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tracing::info;
use windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext;
use winxime_ipc::{check_server_running, IpcClient};
use xime_config::{init_logging_with_console, XimeConfig};
use xime_rime::RimeEngine;

fn main() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }

    init_logging_with_console("server");
    info!("Server starting");
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "/q" || arg == "/quit") {
        if check_server_running() {
            IpcClient::shutdown_server();
        }
        info!("Server stopped via /q");
        return;
    }

    if check_server_running() {
        info!("Stopping existing server...");
        IpcClient::shutdown_server();
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if !check_server_running() {
                break;
            }
        }
        if check_server_running() {
            info!("Failed to stop existing server, exiting");
            return;
        }
        info!("Existing server stopped");
    }

    let (shared_data_dir, user_data_dir, install_user_data_dir) = get_data_dirs();
    info!(
        "Data dirs: shared={}, user={}",
        shared_data_dir.display(),
        user_data_dir.display()
    );

    if !shared_data_dir.exists() {
        info!("Shared data not found at {:?}", shared_data_dir);
        std::process::exit(1);
    }

    let _ = std::fs::create_dir_all(&user_data_dir);
    ensure_user_config_files(&shared_data_dir, &user_data_dir, &install_user_data_dir);

    register::ensure_registered();

    let config = XimeConfig::load();
    let engine = match RimeEngine::new(&shared_data_dir, &user_data_dir, "Xime") {
        Ok(mut e) => {
            e.set_option("_horizontal", config.style.horizontal);
            info!(
                "Rime initialized successfully with horizontal={}",
                config.style.horizontal
            );

            info!("Running rime deployment...");
            if e.deploy() {
                info!("Rime deployment completed successfully");
            } else {
                info!("Rime deployment failed (may already be deployed)");
            }

            if let Some(status) = e.get_status() {
                info!(
                    "Active schema: {} ({})",
                    status.schema_id, status.schema_name
                );
            }

            // Copy compiled dictionary files from shared to user data dir
            // Rime's deploy may skip compiling files whose sources haven't changed,
            // leaving some .table.bin / .reverse.bin files missing in user data dir.
            let shared_build = shared_data_dir.join("build");
            let user_build = user_data_dir.join("build");
            if let Ok(entries) = std::fs::read_dir(&shared_build) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.ends_with(".table.bin") || name_str.ends_with(".reverse.bin") {
                        let user_path = user_build.join(&name);
                        if !user_path.exists() {
                            info!("Copying missing dictionary: {}", name_str);
                            let _ = std::fs::copy(entry.path(), &user_path);
                        }
                    }
                }
            }

            Arc::new(std::sync::Mutex::new(e))
        }
        Err(e) => {
            info!("Rime init failed: {}", e);
            std::process::exit(1);
        }
    };

    run_server(engine);
}

fn get_data_dirs() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    #[cfg(debug_assertions)]
    {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_dir = manifest_dir.parent().unwrap().parent().unwrap();
        (
            workspace_dir.join("rime-wubi"),
            workspace_dir.join("target").join("debug").join("user-data"),
            workspace_dir.join("rime-wubi"),
        )
    }

    #[cfg(not(debug_assertions))]
    {
        let exe_path = std::env::current_exe().ok().unwrap_or_else(|| {
            std::path::PathBuf::from("C:\\Program Files\\Xime\\winxime-server.exe")
        });
        let exe_dir = exe_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("C:\\Program Files\\Xime"));

        let user_data_dir = std::env::var("APPDATA")
            .ok()
            .map(|p| std::path::PathBuf::from(p).join("Xime").join("rime"))
            .unwrap_or_else(|| exe_dir.join("user-data"));

        (exe_dir.join("data"), user_data_dir, exe_dir.join("user-data"))
    }
}

fn ensure_user_config_files(shared_data_dir: &std::path::Path, user_data_dir: &std::path::Path, install_user_data_dir: &std::path::Path) {
    if !install_user_data_dir.exists() || !shared_data_dir.exists() {
        return;
    }

    let _ = std::fs::create_dir_all(user_data_dir);

    let has_yaml = std::fs::read_dir(user_data_dir)
        .map(|entries| {
            entries.flatten().any(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map_or(false, |ext| ext == "yaml")
            })
        })
        .unwrap_or(false);

    if !has_yaml {
        tracing::info!("Deploying schema files from {:?} to {:?} and {:?}", install_user_data_dir, shared_data_dir, user_data_dir);
        copy_dir_contents(install_user_data_dir, shared_data_dir);
        copy_dir_contents(install_user_data_dir, user_data_dir);
    }
}

fn copy_dir_contents(src: &std::path::Path, dst: &std::path::Path) {
    if let Ok(entries) = std::fs::read_dir(src) {
        for entry in entries.flatten() {
            let ft = entry.file_type().ok();
            let dest = dst.join(entry.file_name());
            if ft.map_or(false, |t| t.is_dir()) {
                let _ = std::fs::create_dir_all(&dest);
                copy_dir_recursive(&entry.path(), &dest);
            } else {
                let _ = std::fs::copy(entry.path(), &dest);
            }
        }
    }
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) {
    if let Ok(entries) = std::fs::read_dir(src) {
        for entry in entries.flatten() {
            let ft = entry.file_type().ok();
            let dest = dst.join(entry.file_name());
            if ft.map_or(false, |t| t.is_dir()) {
                let _ = std::fs::create_dir_all(&dest);
                copy_dir_recursive(&entry.path(), &dest);
            } else {
                let _ = std::fs::copy(entry.path(), &dest);
            }
        }
    }
}

fn run_server(engine: Arc<std::sync::Mutex<RimeEngine>>) {
    info!("run_server: starting");
    let context = Arc::new(SharedInputContext::new());
    let ascii_mode = Arc::new(AtomicBool::new(false));
    let main_thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };

    // Determine market directory alongside user data
    let (_, user_data_dir) = xime_config::get_data_dirs();
    let market_dir = user_data_dir.parent().map_or_else(
        || {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("market")
        },
        |parent| parent.join("market"),
    );
    let _ = std::fs::create_dir_all(&market_dir);
    info!("Market directory: {}", market_dir.display());

    let schema_mgr = Arc::new(schema_manager::SchemaManager::new(
        market_dir,
        user_data_dir,
    ));

    info!("Creating UI window...");
    let window = ui::CandidateWindow::new();
    info!("UI window created");

    info!("Starting IPC thread...");
    let engine_clone = engine.clone();
    let context_clone = context.clone();
    let window_clone = window.clone();
    let ascii_mode_clone = ascii_mode.clone();
    let schema_mgr_clone = schema_mgr.clone();
    std::thread::spawn(move || {
        ipc_server::run_ipc_server(
            engine_clone,
            context_clone,
            window_clone,
            ascii_mode_clone,
            main_thread_id,
            schema_mgr_clone,
        );
    });
    info!("IPC thread started");

    info!("Creating tray icon...");
    let on_action = {
        let engine = engine.clone();
        Arc::new(move |action: tray::TrayAction| match action {
            tray::TrayAction::ToggleAsciiMode => {
                if let Ok(mut eng) = engine.try_lock() {
                    let current = eng.is_ascii_mode();
                    eng.set_option("ascii_mode", !current);
                    tray::update_tray_icon(!current);
                }
            }
            tray::TrayAction::OpenSettings => {
                let exe_path = std::env::current_exe().ok().unwrap_or_else(|| {
                    std::path::PathBuf::from(
                        "C:\\Program Files\\winxime-server\\winxime-server.exe",
                    )
                });
                let exe_dir = exe_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("C:\\Program Files\\winxime-server"));
                let setup_path = exe_dir.join("winxime-setup.exe");
                if setup_path.exists() {
                    std::process::Command::new(&setup_path).spawn().ok();
                }
            }
            tray::TrayAction::About => {
                let exe_path = std::env::current_exe().ok().unwrap_or_else(|| {
                    std::path::PathBuf::from(
                        "C:\\Program Files\\winxime-server\\winxime-server.exe",
                    )
                });
                let exe_dir = exe_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("C:\\Program Files\\winxime-server"));
                let setup_path = exe_dir.join("winxime-setup.exe");
                let _ = std::process::Command::new(&setup_path)
                    .arg("--about")
                    .spawn();
            }
            tray::TrayAction::Feedback => {
                let _ = std::process::Command::new("cmd")
                    .args([
                        "/C",
                        "start",
                        "https://github.com/kingzcheung/winxime/issues",
                    ])
                    .spawn();
            }
            tray::TrayAction::Quit => {
                IpcClient::shutdown_server();
            }
        })
    };

    tray::TrayIcon::new(on_action);
    info!("Tray icon created");

    info!("Starting clipboard sharing server...");
    ximed_server::start();
    info!("Clipboard sharing server started");

    info!("Server ready, entering message loop");

    unsafe {
        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        while windows::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
        }
    }
    info!("Message loop exited, cleaning up");

    tray::cleanup();
    info!("Server shutdown complete");
}
