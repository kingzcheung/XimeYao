#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use winxime_ipc::IpcClient;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use xime_setup_lib::{set_app_metadata, AppMetadata};

fn main() {
    let _ = set_app_metadata(AppMetadata {
        display_name: "曦码·曜",
        config_dir_name: "xime",
        config_file_base: "xime",
        distribution_name: "Xime Yao",
        distribution_code_name: "Xime Yao",
        app_name: "rime.xime.setup",
        version: env!("CARGO_PKG_VERSION"),
    });
    const MUTEX_NAME: &str = "XimeSetupSingleInstanceMutex";
    const WINDOW_CLASS: &str = "GPUI Window";
    const WINDOW_TITLE: &str = "曦码·曜 设置";

    let mutex_name_wide: Vec<u16> = MUTEX_NAME
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let already_running = unsafe {
        let handle = CreateMutexW(None, false, PCWSTR(mutex_name_wide.as_ptr()));
        if handle.is_ok() {
            let last_error = GetLastError();
            if last_error == ERROR_ALREADY_EXISTS {
                let class_wide: Vec<u16> = WINDOW_CLASS
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let title_wide: Vec<u16> = WINDOW_TITLE
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let hwnd = FindWindowW(PCWSTR(class_wide.as_ptr()), PCWSTR(title_wide.as_ptr()));
                if hwnd.is_ok() {
                    let hwnd = hwnd.unwrap();
                    if !hwnd.0.is_null() {
                        if IsIconic(hwnd).as_bool() {
                            let _ = ShowWindow(hwnd, SW_RESTORE);
                        }
                        let _ = SetForegroundWindow(hwnd);
                    }
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    };

    if already_running {
        return;
    }

    xime_setup_lib::set_notify_select_schema(|schema_id| {
        IpcClient::select_schema(schema_id)
    });
    xime_setup_lib::set_notify_deploy(|| {
        let _ = IpcClient::reload_config();
    });

    let _ = xime_setup_lib::run();
}
