use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct SchemaManager {
    market_dir: PathBuf,
    user_data_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Registry {
    packages: HashMap<String, PackageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PackageEntry {
    files: Vec<String>,
}

impl SchemaManager {
    pub fn new(market_dir: PathBuf, user_data_dir: PathBuf) -> Self {
        Self {
            market_dir,
            user_data_dir,
        }
    }

    fn registry_path(&self) -> PathBuf {
        self.market_dir.join(".registry.yaml")
    }

    fn load_registry(&self) -> Registry {
        let path = self.registry_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(r) = serde_yaml::from_str(&content) {
                    return r;
                }
            }
        }
        Registry {
            packages: HashMap::new(),
        }
    }

    fn save_registry(&self, registry: &Registry) {
        if let Ok(yaml) = serde_yaml::to_string(registry) {
            let _ = std::fs::write(self.registry_path(), yaml);
        }
    }

    pub fn market_dir(&self) -> &Path {
        &self.market_dir
    }

    pub fn schema_package_dir(&self, schema_id: &str) -> PathBuf {
        self.market_dir.join(schema_id)
    }

    pub fn fetch_index(&self) -> Result<String, String> {
        let urls = [
            "https://index.ximei.me/rimes/index.yaml",
            "https://cdn.jsdelivr.net/gh/ximeiorg/xime-index@master/rimes/index.yaml",
            "https://raw.githubusercontent.com/ximeiorg/xime-index/refs/heads/main/rimes/index.yaml",
        ];
        for url in &urls {
            match self.fetch_url(url) {
                Ok(text) => return Ok(text),
                Err(_) => continue,
            }
        }
        Err("所有镜像源都无法连接".to_string())
    }

    fn fetch_url(&self, url: &str) -> Result<String, String> {
        let response = ureq::get(url)
            .call()
            .map_err(|e| format!("网络请求失败: {}", e))?;
        response
            .into_body()
            .read_to_string()
            .map_err(|e| format!("读取响应失败: {}", e))
    }

    pub fn download_schema(
        &self,
        schema_id: &str,
        url: &str,
        sha256: Option<&str>,
        filename: &str,
    ) -> Result<(), String> {
        let pkg_dir = self.schema_package_dir(schema_id);
        std::fs::create_dir_all(&pkg_dir)
            .map_err(|e| format!("创建目录失败: {}", e))?;

        let response =
            ureq::get(url)
                .call()
                .map_err(|e| format!("下载失败: {}", e))?;

        let bytes = response
            .into_body()
            .read_to_vec()
            .map_err(|e| format!("读取下载内容失败: {}", e))?;

        if let Some(expected) = sha256 {
            let actual = hex::encode(Sha256::digest(&bytes));
            if actual != expected {
                return Err(format!(
                    "SHA256 校验失败: 期望 {}，实际 {}",
                    expected, actual
                ));
            }
        }

        let dest = pkg_dir.join(filename);
        std::fs::write(&dest, &bytes).map_err(|e| format!("写入文件失败: {}", e))?;

        Ok(())
    }

    pub fn install_schema(&self, schema_id: &str) -> Result<(), String> {
        let pkg_dir = self.schema_package_dir(schema_id);
        if !pkg_dir.exists() {
            return Err(format!("方案 '{}' 未下载", schema_id));
        }

        let archive = std::fs::read_dir(&pkg_dir)
            .map_err(|e| format!("读取下载目录失败: {}", e))?
            .flatten()
            .find(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                n.ends_with(".zip") || n.ends_with(".tar.gz") || n.ends_with(".tgz")
            })
            .ok_or_else(|| format!("未找到压缩包"))?;

        let archive_path = archive.path();
        let temp_dir = std::env::temp_dir().join(format!("xime_extract_{}", schema_id));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("创建临时目录失败: {}", e))?;

        let filename = archive_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        if filename.ends_with(".zip") {
            Self::extract_zip(&archive_path, &temp_dir)?;
        } else {
            Self::extract_tar_gz(&archive_path, &temp_dir)?;
        }

        let schema_files = self.find_schema_files(&temp_dir);
        if schema_files.is_empty() {
            return Err("下载包中未找到 .schema.yaml 文件".to_string());
        }

        let mut installed_files = Vec::new();
        for path in &schema_files {
            let name = path.file_name().unwrap();
            let dest = self.user_data_dir.join(name);
            std::fs::copy(path, &dest)
                .map_err(|e| format!("复制文件失败: {}", e))?;
            installed_files.push(name.to_string_lossy().to_string());
        }

        let mut registry = self.load_registry();
        registry
            .packages
            .insert(schema_id.to_string(), PackageEntry {
                files: installed_files,
            });
        self.save_registry(&registry);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    pub fn uninstall_schema(&self, schema_id: &str) -> Result<(), String> {
        let mut registry = self.load_registry();
        let entry = registry
            .packages
            .remove(schema_id)
            .ok_or_else(|| format!("方案 '{}' 未通过市场安装", schema_id))?;

        for file in &entry.files {
            let path = self.user_data_dir.join(file);
            let _ = std::fs::remove_file(&path);
        }

        self.save_registry(&registry);
        Ok(())
    }

    pub fn list_market_schemas(&self) -> Vec<String> {
        let mut ids = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.market_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        if !name.starts_with('.') {
                            ids.push(name.to_string());
                        }
                    }
                }
            }
        }
        ids
    }

    pub fn list_installed_packages(&self) -> Vec<String> {
        let registry = self.load_registry();
        registry.packages.keys().cloned().collect()
    }

    fn extract_zip(archive_path: &Path, dest: &Path) -> Result<(), String> {
        let file = std::fs::File::open(archive_path).map_err(|e| format!("打开压缩包失败: {}", e))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("读取 zip 失败: {}", e))?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| format!("读取 zip 条目失败: {}", e))?;
            let Some(path) = entry
                .enclosed_name()
                .map(|p| p.to_path_buf())
            else {
                continue;
            };
            let target = dest.join(&path);
            if entry.is_dir() {
                std::fs::create_dir_all(&target).ok();
            } else {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                let mut output =
                    std::fs::File::create(&target).map_err(|e| format!("创建文件失败: {}", e))?;
                std::io::copy(&mut entry, &mut output)
                    .map_err(|e| format!("解压失败: {}", e))?;
            }
        }
        Ok(())
    }

    fn extract_tar_gz(archive_path: &Path, dest: &Path) -> Result<(), String> {
        let file =
            std::fs::File::open(archive_path).map_err(|e| format!("打开压缩包失败: {}", e))?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        for entry in archive.entries().map_err(|e| format!("读取 tar 失败: {}", e))? {
            let mut entry = entry.map_err(|e| format!("读取 tar 条目失败: {}", e))?;
            let path = entry.path().map_err(|e| format!("读取路径失败: {}", e))?.to_path_buf();
            let target = dest.join(&path);
            if entry.header().entry_type().is_dir() {
                std::fs::create_dir_all(&target).ok();
            } else {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                entry
                    .unpack(&target)
                    .map_err(|e| format!("解压失败: {}", e))?;
            }
        }
        Ok(())
    }

    fn find_schema_files(&self, dir: &Path) -> Vec<PathBuf> {
        let mut results = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    results.extend(self.find_schema_files(&path));
                } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".schema.yaml") {
                        results.push(path);
                    }
                }
            }
        }
        results
    }
}
