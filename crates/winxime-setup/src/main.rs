#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use winxime_ipc::IpcClient;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use xime_setup_lib::{set_app_metadata, AppMetadata};

mod toast;

fn main() {
    // 设置进程日志（%APPDATA%\xime\logs\setup.log；GUI 进程无控制台可看）。
    xime_config::init_logging_with_console("setup");

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
    // 剪贴板同步插件启停/选择后通知 server 重载插件运行时。
    xime_setup_lib::set_notify_reload_plugins(|| {
        let _ = IpcClient::reload_plugins();
    });
    // 用户资料同步：IPC SyncUserData（阻塞等待 rime 维护线程完成）。
    xime_setup_lib::set_notify_sync_user_data(IpcClient::sync_user_data);
    // 词典管理：rime 用户词典操作（经 IPC 在 server 进程执行）。
    xime_setup_lib::set_notify_dict_list(|| {
        IpcClient::list_user_dicts().map(|d| xime_setup_lib::state::DictListResult {
            dicts: d.dicts,
            sync_dir: d.sync_dir,
        })
    });
    xime_setup_lib::set_notify_dict_backup(IpcClient::backup_user_dict);
    xime_setup_lib::set_notify_dict_restore(IpcClient::restore_user_dict);
    xime_setup_lib::set_notify_dict_export(IpcClient::export_user_dict);
    xime_setup_lib::set_notify_dict_import(IpcClient::import_user_dict);
    // 部署结果系统通知（WinRT toast；非打包环境静默跳过）。
    xime_setup_lib::set_notify_deploy_toast(toast::show_toast);

    let _ = xime_setup_lib::run();
}
