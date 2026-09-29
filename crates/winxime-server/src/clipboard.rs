//! 系统剪贴板访问与监听。
//!
//! 监听窗口为 HWND_MESSAGE 消息窗口（不抢焦点不显示）：`WM_CLIPBOARDUPDATE`
//! 通知本地变化，`SetTimer` 定时触发远端拉取节拍。事件回调在 UI 线程触发，
//! 调用方自行把阻塞操作（插件 HTTP）派发到工作线程。

use std::sync::Arc;

use tracing::{error, info};
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, EnumClipboardFormats,
    GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, RegisterClassW, SetTimer, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_CLIPBOARDUPDATE, WM_DESTROY, WM_TIMER, WNDCLASSW, HWND_MESSAGE,
};
use windows_core::{w, PCWSTR};

/// 远端剪贴板拉取节拍。
const TIMER_PULL_MS: u32 = 30_000;

/// OpenClipboard 重试：复制方应用可能仍短暂持有剪贴板锁，
/// WM_CLIPBOARDUPDATE 到达时立即打开常失败，需小间隔重试。
const CLIPBOARD_OPEN_RETRIES: usize = 5;
const CLIPBOARD_OPEN_RETRY_MS: u64 = 10;

/// 以重试方式打开剪贴板（关联监听窗口句柄）；全部失败返回 false。
unsafe fn open_clipboard_with_retry() -> bool {
    let hwnd_value = LISTENER_HWND.load(std::sync::atomic::Ordering::Acquire);
    let owner = if hwnd_value == 0 {
        None
    } else {
        Some(HWND(hwnd_value as *mut core::ffi::c_void))
    };
    for attempt in 0..CLIPBOARD_OPEN_RETRIES {
        if OpenClipboard(owner).is_ok() {
            return true;
        }
        if attempt + 1 < CLIPBOARD_OPEN_RETRIES {
            std::thread::sleep(std::time::Duration::from_millis(CLIPBOARD_OPEN_RETRY_MS));
        }
    }
    tracing::warn!(
        "打开剪贴板失败（重试 {} 次仍被占用）",
        CLIPBOARD_OPEN_RETRIES
    );
    false
}

/// 剪贴板事件：本地内容变化 / 定时拉取节拍。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardEvent {
    Changed,
    Tick,
}

type EventCallback = Arc<dyn Fn(ClipboardEvent) + Send + Sync>;

static mut CLIPBOARD_CALLBACK: Option<EventCallback> = None;

/// 监听窗口句柄（OpenClipboard 传真实窗口而非 NULL：
/// 消息循环线程上 NULL 关联的打开状态不可靠，GetClipboardData 会报
/// 「线程没有打开的剪贴板」；剪贴板管理器的常规做法是传窗口句柄）。
static LISTENER_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// 读取剪贴板 UTF-16 文本；无文本或类型不符返回 None。
pub fn read_text() -> Option<String> {
    unsafe {
        if !open_clipboard_with_retry() {
            return None;
        }
        let result = (|| {
            let handle = match GetClipboardData(CF_UNICODETEXT.0 as u32) {
                Ok(h) => h,
                Err(e) => {
                    // 枚举剪贴板上实际存在的格式（定位“复制的内容无文本格式”类问题）
                    let mut formats = Vec::new();
                    let mut fmt = EnumClipboardFormats(0);
                    while fmt != 0 {
                        formats.push(fmt);
                        fmt = EnumClipboardFormats(fmt);
                    }
                    tracing::warn!(
                        "剪贴板无 CF_UNICODETEXT 数据 (error={:?})，实际格式: {:?}",
                        e,
                        formats
                    );
                    return None;
                }
            };
            let global = HGLOBAL(handle.0);
            let ptr = GlobalLock(global) as *const u16;
            if ptr.is_null() {
                tracing::warn!("剪贴板 GlobalLock 失败");
                return None;
            }
            // 以双零结尾的 UTF-16 序列
            let mut len = 0usize;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(ptr, len);
            let _ = GlobalUnlock(global);
            String::from_utf16_lossy(slice).into()
        })();
        let _ = CloseClipboard();
        result
    }
}

/// 写入 UTF-16 文本到剪贴板；成功后剪贴板接管内存所有权。
pub fn write_text(text: &str) -> bool {
    unsafe {
        if !open_clipboard_with_retry() {
            return false;
        }
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        let bytes_len = wide.len() * 2;
        let result = (|| -> Option<bool> {
            let _ = EmptyClipboard();
            let global = GlobalAlloc(GMEM_MOVEABLE, bytes_len).ok()?;
            let ptr = GlobalLock(global) as *mut u16;
            if ptr.is_null() {
                return Some(false);
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
            let _ = GlobalUnlock(global);
            Some(SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(global.0))).is_ok())
        })()
        .unwrap_or(false);
        let _ = CloseClipboard();
        result
    }
}

/// 创建剪贴板监听窗口（HWND_MESSAGE 消息窗口）、注册监听与拉取定时器。
/// 必须在运行消息循环的线程上调用；消息由宿主既有消息循环分发，本函数立即返回。
pub fn start_listener(callback: EventCallback) {
    unsafe {
        // 回调经静态存储（与 tray.rs 同一模式），wnd_proc 直接读取。
        CLIPBOARD_CALLBACK = Some(callback);

        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("XimeClipboardListenerWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);

        match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            w!("XimeClipboardListener"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(hinstance.into()),
            None,
        ) {
            Ok(hwnd) => {
                LISTENER_HWND.store(
                    hwnd.0 as isize,
                    std::sync::atomic::Ordering::Release,
                );
                let _ = AddClipboardFormatListener(hwnd);
                let _ = SetTimer(Some(hwnd), 1, TIMER_PULL_MS, None);
                info!("剪贴板监听已启动");
            }
            Err(e) => error!("剪贴板监听窗口创建失败: {:?}", e),
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CLIPBOARDUPDATE => {
            fire(ClipboardEvent::Changed);
            LRESULT(0)
        }
        WM_TIMER => {
            fire(ClipboardEvent::Tick);
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn fire(event: ClipboardEvent) {
    tracing::debug!("剪贴板事件触发: {:?}", event);
    let callback = unsafe {
        #[allow(static_mut_refs)]
        CLIPBOARD_CALLBACK.as_ref().map(Arc::clone)
    };
    if let Some(callback) = callback {
        callback(event);
    } else {
        tracing::warn!("剪贴板事件无回调（监听未初始化）");
    }
}
