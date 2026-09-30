//! Windows 系统通知（WinRT ToastNotification）。
//!
//! 仅在 MSIX 包内运行时可用（包进程有 package identity，AUMID =
//! `{PackageFullName}!{AppId}`）；非打包环境（开发直跑）静默跳过，
//! 页面底部消息不受影响。

#![cfg(windows)]

use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows_core::{HSTRING, PWSTR};

/// MSIX 清单里的应用入口 Id（crates/winxime-server/msix/AppxManifest.xml）。
const PACKAGE_APP_ID: &str = "XimeServer";

/// 弹出系统通知；失败静默（通知属尽力而为的提示，不阻塞设置操作）。
pub fn show_toast(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    let _ = std::thread::Builder::new()
        .name("toast".into())
        .spawn(move || show_toast_blocking(title, body));
}

fn show_toast_blocking(title: String, body: String) {
    // 后台线程调 WinRT 需先初始化 COM 单元（已初始化则忽略）。
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let Some(aumid) = package_aumid() else {
        return;
    };
    let xml = format!(
        "<toast activationType=\"system\" launch=\"\">\
<visual><binding template=\"ToastGeneric\">\
<text>{}</text><text>{}</text>\
</binding></visual></toast>",
        xml_escape(&title),
        xml_escape(&body),
    );
    let result = (|| -> windows_core::Result<()> {
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&doc)?;
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(aumid))?;
        notifier.Show(&toast)
    })();
    if let Err(e) = result {
        tracing::warn!("系统通知显示失败: {e}");
    }
}

/// 当前包身份的 AUMID；非打包进程返回 None。
fn package_aumid() -> Option<String> {
    use windows::Win32::Foundation::{APPMODEL_ERROR_NO_PACKAGE, ERROR_INSUFFICIENT_BUFFER};
    unsafe {
        // Win32 两段式：首传空缓冲取长度（打包进程返回 INSUFFICIENT_BUFFER，
        // 非打包进程返回 NO_PACKAGE），再传缓冲取全名。
        let mut len = 0u32;
        let err = GetCurrentPackageFullName(&mut len, None);
        if err == APPMODEL_ERROR_NO_PACKAGE {
            return None;
        }
        if err != ERROR_INSUFFICIENT_BUFFER && err.0 != 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        let err = GetCurrentPackageFullName(&mut len, Some(PWSTR(buf.as_mut_ptr())));
        if err.0 != 0 {
            return None;
        }
        let pfn = String::from_utf16_lossy(&buf[..len as usize]);
        Some(format!("{pfn}!{PACKAGE_APP_ID}"))
    }
}

/// XML 文本转义（& < > " '）。
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
