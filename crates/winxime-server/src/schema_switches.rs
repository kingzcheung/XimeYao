//! 当前方案 `switches` 读取（对齐 Android `SchemaManager.parseSchemaSwitches`）。
//!
//! 供托盘菜单动态渲染：布尔开关（name + states 两态标签）与多选一开关
//! （options 选项轮转 + states 对应标签）。只解析映射条目，字符串简写
//! 条目（如 `- ascii_mode`）与 Android 端一致直接跳过。

use std::path::Path;

/// 方案开关定义（对齐 Android `SchemaSwitch`）。
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaSwitch {
    /// 布尔开关名；多选一开关为空。
    pub name: String,
    /// 多选一 option 名列表（点击轮转，激活项 = 第一个取 true 的 option）。
    pub options: Vec<String>,
    /// 各状态展示标签（与 options 一一对应；布尔开关为 [关, 开]）。
    pub states: Vec<String>,
}

/// 读取方案 switches：优先 rime 根目录的 `<id>.schema.yaml`（部署副本，
/// 已含 custom 补丁合并后的形态），缺失时回退 build/ 编译产物。
pub fn read_schema_switches(rime_dir: &Path, schema_id: &str) -> Vec<SchemaSwitch> {
    let candidates = [
        rime_dir.join(format!("{schema_id}.schema.yaml")),
        rime_dir
            .join("build")
            .join(format!("{schema_id}.schema.yaml")),
    ];
    for path in candidates {
        if let Ok(text) = std::fs::read_to_string(&path) {
            match parse_switches(&text) {
                Some(list) => return list,
                None => tracing::warn!("方案 switches 解析失败: {}", path.display()),
            }
        }
    }
    Vec::new()
}

/// 解析 switches 块；无 switches 段返回 None（调用方继续尝试下个候选文件）。
fn parse_switches(text: &str) -> Option<Vec<SchemaSwitch>> {
    let value: serde_yaml::Value = serde_yaml::from_str(text).ok()?;
    let list = value.get("switches")?.as_sequence()?;
    let mut out = Vec::new();
    for item in list {
        let Some(map) = item.as_mapping() else {
            continue;
        };
        let name = map
            .get(serde_yaml::Value::from("name"))
            .and_then(yaml_str)
            .unwrap_or_default();
        let states: Vec<String> = map
            .get(serde_yaml::Value::from("states"))
            .and_then(|v| v.as_sequence())
            .map(|seq| seq.iter().filter_map(yaml_str).collect())
            .unwrap_or_default();
        if states.is_empty() {
            continue;
        }
        let options: Vec<String> = map
            .get(serde_yaml::Value::from("options"))
            .and_then(|v| v.as_sequence())
            .map(|seq| seq.iter().filter_map(yaml_str).collect())
            .unwrap_or_default();
        if !name.is_empty() {
            out.push(SchemaSwitch {
                name,
                options: Vec::new(),
                states,
            });
        } else if !options.is_empty() {
            out.push(SchemaSwitch {
                name: String::new(),
                options,
                states,
            });
        }
    }
    Some(out)
}

fn yaml_str(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
schema:
  schema_id: wubi86
switches:
  - name: ascii_mode
    reset: 0
    states: [ 中文, 西文 ]
  - name: full_shape
    states: [ 半角, 全角 ]
  - name: extended_charset
    states: [ 常用, 增广 ]
  - options: [ simplified, traditional ]
    states: [ 简体, 繁體 ]
  - emoji
"#;

    #[test]
    fn parse_switches_covers_bool_and_options() {
        let switches = parse_switches(SAMPLE).expect("switches 段存在");
        assert_eq!(switches.len(), 4, "字符串简写条目应被跳过");

        assert_eq!(switches[0].name, "ascii_mode");
        assert_eq!(switches[0].states, vec!["中文", "西文"]);
        assert!(switches[0].options.is_empty());

        assert_eq!(switches[2].name, "extended_charset");

        let multi = &switches[3];
        assert!(multi.name.is_empty());
        assert_eq!(multi.options, vec!["simplified", "traditional"]);
        assert_eq!(multi.states, vec!["简体", "繁體"]);
    }

    #[test]
    fn parse_switches_without_section_returns_none() {
        assert!(parse_switches("schema:\n  schema_id: x\n").is_none());
    }

    #[test]
    fn read_schema_switches_missing_files_returns_empty() {
        let dir = std::env::temp_dir().join(format!("xime_switches_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(read_schema_switches(&dir, "nonexistent").is_empty());
    }

    #[test]
    fn read_schema_switches_reads_deployed_file() {
        let dir = std::env::temp_dir().join(format!("xime_switches_rd_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("wubi86.schema.yaml"), SAMPLE).unwrap();
        let switches = read_schema_switches(&dir, "wubi86");
        assert_eq!(switches.len(), 4);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
