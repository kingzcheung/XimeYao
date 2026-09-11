//! 插件宿主（对齐 Xime plugin-core 的宿主侧职责分工）：
//! - 宿主做重活：内置插件安装、Lua 运行时加载、备份包打包/恢复、剪贴板监听与去重
//! - 插件只做协议传输（WebDAV PUT/GET/PROPFIND/DELETE，经 host.http/host.crypto/host.config）
//!
//! 目录布局（对齐 Xime 的 filesDir 布局）：
//! - `%APPDATA%\Xime\rime`           rime 用户数据（单目录模型）
//! - `%APPDATA%\Xime\plugins\<id>`   已安装插件
//! - `%APPDATA%\Xime\plugins\registry.yaml`
//! - `%APPDATA%\Xime\plugins\config\<id>.yaml` 插件配置（host.config 存取）

use std::collections::HashMap;
use std::io::{Cursor, Write, Seek};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tracing::{info, warn};
use xime_plugin::{PluginManager, PluginManifest, PluginRecordState, PluginRuntime};

struct LoadedPlugin {
    manifest: PluginManifest,
    runtime: PluginRuntime,
}

pub struct PluginHost {
    manager: PluginManager,
    runtimes: Mutex<HashMap<String, LoadedPlugin>>,
    /// rime 用户数据目录（单目录模型）：备份打包/恢复的对象。
    rime_dir: PathBuf,
    // 剪贴板同步三通道去重（对齐 Xime ClipboardSyncBridge 语义）。
    clipboard_current: Mutex<Option<String>>,
    clipboard_last_pushed: Mutex<Option<String>>,
    clipboard_self_written: Mutex<Option<String>>,
}

impl PluginHost {
    /// 创建插件宿主：安装内置插件并加载全部已启用插件。
    /// `bundled_dir` 为安装目录自带的插件源（`resources/plugins`），缺失时跳过安装。
    pub fn new(rime_dir: PathBuf, bundled_dir: Option<PathBuf>) -> Arc<Self> {
        let plugins_root = rime_dir
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("plugins");
        let manager = PluginManager::new(&plugins_root);

        if let Some(bundled) = bundled_dir {
            install_bundled(&manager, &bundled);
        }

        let host = Arc::new(Self {
            manager,
            runtimes: Mutex::new(HashMap::new()),
            rime_dir,
            clipboard_current: Mutex::new(None),
            clipboard_last_pushed: Mutex::new(None),
            clipboard_self_written: Mutex::new(None),
        });
        host.load_enabled();
        host
    }

    fn load_enabled(&self) {
        let mut map = self.runtimes.lock().unwrap_or_else(|e| e.into_inner());
        for record in self.manager.list() {
            if !record.enabled || record.state != PluginRecordState::Ready {
                continue;
            }
            let dir = self.manager.plugin_dir(&record.id);
            let manifest = match PluginManifest::from_dir(&dir) {
                Ok(m) => m,
                Err(e) => {
                    warn!("插件 manifest 读取失败 {}: {}", record.id, e);
                    continue;
                }
            };
            let config = self.manager.config_path(&record.id);
            match PluginRuntime::load(&dir, &manifest.entry, &config) {
                Ok(runtime) => {
                    runtime.call_on_load();
                    info!(
                        "插件已加载: {} v{} (type={})",
                        record.id, record.version, manifest.plugin_type
                    );
                    map.insert(record.id, LoadedPlugin { manifest, runtime });
                }
                Err(e) => {
                    warn!("插件加载失败 {}: {}", record.id, e);
                }
            }
        }
    }

    // ---- 云备份（宿主打包，插件传输）----

    /// 打包 rime 用户数据为 zip（跳过可再生的 build/ 目录）。
    /// 条目前缀 `rime/`，与 Xime 的备份包布局一致（恢复时去掉前缀落回 rime 目录）。
    pub fn build_backup_archive(&self) -> Result<(String, Vec<u8>), String> {
        let file_name = format!("Xime备份-{}.zip", local_date_string());

        let file = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        add_dir_to_zip(&mut zip, &self.rime_dir, "rime", options)?;

        let cursor = zip
            .finish()
            .map_err(|e| format!("zip 收尾失败: {}", e))?;
        Ok((file_name, cursor.into_inner()))
    }

    /// 立即备份：打包 → 找 backup 类插件推送。返回远端条目 id。
    pub fn backup_now(&self) -> Result<String, String> {
        let (name, archive) = self.build_backup_archive()?;
        self.with_typed_runtime("backup", |runtime| {
            match runtime.backup_push(&name, &archive) {
                Some(result) if result.get("ok").and_then(|b| b.as_bool()).unwrap_or(false) => {
                    Ok(result
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string())
                }
                Some(result) => {
                    let message = result
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("未知错误");
                    Err(format!("插件返回失败: {}", message))
                }
                None => Err("插件推送失败".to_string()),
            }
        })
        .ok_or_else(|| "未安装已启用的 backup 类插件".to_string())?
    }

    /// 列出远端备份条目（[{id, name, createdAt, size}]）。
    pub fn list_backups(&self) -> Result<Vec<serde_json::Value>, String> {
        self.with_typed_runtime("backup", |runtime| {
            runtime
                .backup_list()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default()
        })
        .ok_or_else(|| "未安装已启用的 backup 类插件".to_string())
    }

    /// 拉取并恢复备份：`_xime_backup/` 元数据条目跳过（设置/插件配置恢复属后续功能点），
    /// 其余条目按 zip 相对路径写回 rime 目录（enclosed_name 防路径穿越）。
    /// 返回恢复的文件数。
    pub fn restore_backup(&self, id: &str) -> Result<usize, String> {
        let bytes = self
            .with_typed_runtime("backup", |runtime| runtime.backup_pull(id))
            .ok_or_else(|| "未安装已启用的 backup 类插件".to_string())?
            .ok_or_else(|| "远端备份不存在或拉取失败".to_string())?;

        let mut archive =
            zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("zip 打开失败: {}", e))?;
        let mut restored = 0usize;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("zip 条目读取失败: {}", e))?;
            let Some(rel) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
                continue;
            };
            let rel_str = rel.to_string_lossy();
            if rel_str.starts_with("_xime_backup/") || rel_str == "_xime_backup" {
                continue;
            }
            // 去掉打包时的 rime/ 前缀
            let target_rel = rel.strip_prefix("rime").unwrap_or(&rel);
            if target_rel.as_os_str().is_empty() {
                continue;
            }
            let dest = self.rime_dir.join(target_rel);
            if entry.is_dir() {
                let _ = std::fs::create_dir_all(&dest);
                continue;
            }
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut out = std::fs::File::create(&dest)
                .map_err(|e| format!("写入 {} 失败: {}", dest.display(), e))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| format!("解压 {} 失败: {}", dest.display(), e))?;
            restored += 1;
        }
        info!("备份恢复完成: {} 个文件", restored);
        Ok(restored)
    }

    /// 删除远端备份条目。
    pub fn delete_backup(&self, id: &str) -> Result<bool, String> {
        self.with_typed_runtime("backup", |runtime| runtime.backup_delete(id))
            .ok_or_else(|| "未安装已启用的 backup 类插件".to_string())?
            .ok_or_else(|| "插件删除失败".to_string())
    }

    // ---- 剪贴板同步（宿主监听+去重，插件传输）----

    pub fn has_clipboard_sync(&self) -> bool {
        self.has_typed_runtime("clipboard_sync")
    }

    /// 本地剪贴板变化：hash 去重后经插件推送到远端。
    pub fn clipboard_local_changed(&self, text: &str) {
        if !self.has_typed_runtime("clipboard_sync") {
            return;
        }
        let hash = sha256_hex(text.as_bytes());
        {
            let mut current = self.clipboard_current.lock().unwrap_or_else(|e| e.into_inner());
            if current.as_deref() == Some(hash.as_str()) {
                return;
            }
            *current = Some(hash.clone());
        }
        let last_pushed = self.clipboard_last_pushed.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let self_written = self.clipboard_self_written.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if Some(&hash) == last_pushed.as_ref() || Some(&hash) == self_written.as_ref() {
            return;
        }

        let profile = serde_json::json!({
            "type": "text",
            "hash": hash,
            "text": text,
            "has_data": false,
            "data_name": null,
            "size": text.len(),
            "source": "xime-windows",
        });
        let pushed = self.with_typed_runtime("clipboard_sync", |runtime| {
            runtime.clipboard_push(&profile)
        });
        if pushed.unwrap_or(false) {
            *self.clipboard_last_pushed.lock().unwrap_or_else(|e| e.into_inner()) = Some(hash);
            info!("剪贴板已推送 ({} 字符)", text.len());
        }
    }

    /// 拉取远端剪贴板：远端内容与本地/自写不同时返回 Some(text)，由调用方写回系统剪贴板。
    pub fn clipboard_pull_remote(&self) -> Option<String> {
        if !self.has_typed_runtime("clipboard_sync") {
            return None;
        }
        let profile = self.with_typed_runtime("clipboard_sync", |runtime| {
            runtime.clipboard_pull()
        })??;
        let text = profile.get("text")?.as_str()?.to_string();
        if text.is_empty() {
            return None;
        }
        // hash 留空（旧版纯文本）时由宿主补算
        let hash = profile
            .get("hash")
            .and_then(|v| v.as_str())
            .filter(|h| !h.is_empty())
            .map(String::from)
            .unwrap_or_else(|| sha256_hex(text.as_bytes()));

        let current = self.clipboard_current.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let self_written = self.clipboard_self_written.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if Some(&hash) == current.as_ref() || Some(&hash) == self_written.as_ref() {
            return None;
        }

        *self.clipboard_self_written.lock().unwrap_or_else(|e| e.into_inner()) = Some(hash);
        *self.clipboard_current.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(sha256_hex(text.as_bytes()));
        Some(text)
    }

    // ---- 内部 ----

    fn has_typed_runtime(&self, plugin_type: &str) -> bool {
        self.runtimes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .any(|p| p.manifest.plugin_type == plugin_type)
    }

    /// 对第一个指定类型的插件运行时执行 `f`（线程内串行，Lua 调用可能阻塞 HTTP）。
    fn with_typed_runtime<T>(
        &self,
        plugin_type: &str,
        f: impl FnOnce(&PluginRuntime) -> T,
    ) -> Option<T> {
        let map = self.runtimes.lock().unwrap_or_else(|e| e.into_inner());
        map.values()
            .find(|p| p.manifest.plugin_type == plugin_type)
            .map(|p| f(&p.runtime))
    }
}

/// 插件包下载临时文件路径（对齐安卓 `cache/xime_plugin_{id}_{fileName}` 约定：
/// 下载 → sha256 校验 → `PluginManager::install_from_zip` → 临时文件即删）。
/// 插件市场下载属后续功能点，先固化路径约定。
#[allow(dead_code)]
pub fn plugin_download_temp_path(plugin_id: &str, file_name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("xime_plugin_{}_{}", plugin_id, file_name))
}

/// 安装安装目录自带的内置插件（resources/plugins/<目录>），覆盖安装但保留启用状态。
fn install_bundled(manager: &PluginManager, bundled_dir: &Path) {    let Ok(entries) = std::fs::read_dir(bundled_dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        match manager.install_from_dir(&entry.path(), true) {
            Ok(record) => info!("内置插件已安装: {} v{}", record.id, record.version),
            Err(e) => warn!("内置插件安装失败 {}: {}", entry.path().display(), e),
        }
    }
}

fn add_dir_to_zip<W: std::io::Write + Seek>(
    zip: &mut zip::ZipWriter<W>,
    dir: &Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("读取 {} 失败: {}", dir.display(), e))?;
    for entry in entries.flatten() {
        let name = format!("{}/{}", prefix, entry.file_name().to_string_lossy());
        match entry.file_type() {
            Ok(t) if t.is_dir() => {
                // build/ 为部署产物，可再生，不进备份包
                if entry.file_name() == "build" {
                    continue;
                }
                add_dir_to_zip(zip, &entry.path(), &name, options)?;
            }
            Ok(t) if t.is_file() => {
                let bytes = std::fs::read(entry.path())
                    .map_err(|e| format!("读取 {} 失败: {}", entry.path().display(), e))?;
                zip.start_file(&name, options)
                    .map_err(|e| format!("zip 写入 {} 失败: {}", name, e))?;
                zip.write_all(&bytes)
                    .map_err(|e| format!("zip 写入 {} 失败: {}", name, e))?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 本地日期 YYYY-MM-DD（civil_from_days，无外部时间依赖）。
fn local_date_string() -> String {
    let days = (unix_secs() / 86400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}
