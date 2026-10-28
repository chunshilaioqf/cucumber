//! Skill 安装（FR-SKILL-01~03，NFR-SEC-03）。
//!
//! skill 形态固定为「目录 + SKILL.md（YAML frontmatter + Markdown 正文）」，不含可执行脚本。
//! 内置 skill 以 `include_str!` 嵌入二进制，安装时落盘到目标目录并维护锁清单。

use std::path::{Path, PathBuf};

/// 内置 skill：黄瓜进程管理。
pub const BUILTIN_SKILL_NAME: &str = "cucumber-manager";
/// SKILL.md 正文（编译期嵌入）。
const BUILTIN_SKILL_MD: &str = include_str!("../../skills/cucumber-manager/SKILL.md");

/// skill 安装记录（.skill-lock.json 条目）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillRecord {
    pub name: String,
    pub version: String,
    pub installed_at: i64,
    /// builtin / 留空表示外部来源
    pub source: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SkillLock {
    pub skills: Vec<SkillRecord>,
}

/// 默认安装目录 ~/.agents/skills（FR-SKILL-02）。
pub fn default_skill_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "无法定位用户主目录")?;
    Ok(PathBuf::from(home).join(".agents").join("skills"))
}

/// 解析 SKILL.md 的 frontmatter（--- 之间的 key: value 行）。
/// 返回字段映射；文件缺 frontmatter 时返回 Err。
pub fn parse_frontmatter(md: &str) -> Result<Vec<(String, String)>, String> {
    let rest = md.trim_start();
    let body = rest
        .strip_prefix("---")
        .ok_or("缺少 frontmatter 起始标记 ---")?;
    let end = body.find("\n---").ok_or("缺少 frontmatter 结束标记 ---")?;
    let block = &body[..end];
    let mut fields = Vec::new();
    for line in block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
            fields.push((k.trim().to_string(), v));
        }
    }
    if fields.is_empty() {
        return Err("frontmatter 为空".into());
    }
    Ok(fields)
}

/// 安装前校验（FR-SKILL-03）：SKILL.md 存在、frontmatter 可解析、无子目录、无可执行文件。
pub fn validate_skill_dir(dir: &Path) -> Result<(), String> {
    let md = dir.join("SKILL.md");
    if !md.is_file() {
        return Err("SKILL.md 不存在".into());
    }
    let text = std::fs::read_to_string(&md).map_err(|e| format!("读取 SKILL.md 失败：{e}"))?;
    parse_frontmatter(&text)?;
    // NFR-SEC-03：拒绝可执行脚本与深层目录
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let p = entry.path();
        if p.is_dir() {
            return Err(format!("skill 目录不允许包含子目录：{}", p.display()));
        }
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if matches!(
            ext.as_str(),
            "exe" | "sh" | "bash" | "bat" | "cmd" | "ps1" | "bin" | "dll" | "so" | "dylib" | "com"
        ) {
            return Err(format!("skill 不允许包含可执行文件：{}", p.display()));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(md) = std::fs::metadata(&p) {
                if md.permissions().mode() & 0o111 != 0 {
                    return Err(format!("skill 文件不允许带可执行位：{}", p.display()));
                }
            }
        }
    }
    Ok(())
}

/// 安装内置 skill 到目标目录；维护 `.skill-lock.json`；重复安装幂等（覆盖更新）。
pub fn install_builtin(target_dir: Option<&Path>) -> Result<PathBuf, String> {
    let dir = match target_dir {
        Some(d) => d.to_path_buf(),
        None => default_skill_dir()?,
    };
    let skill_dir = dir.join(BUILTIN_SKILL_NAME);
    std::fs::create_dir_all(&skill_dir).map_err(|e| format!("创建目录失败：{e}"))?;
    std::fs::write(skill_dir.join("SKILL.md"), BUILTIN_SKILL_MD)
        .map_err(|e| format!("写入 SKILL.md 失败：{e}"))?;
    validate_skill_dir(&skill_dir)?;

    // 锁清单
    let lock_path = dir.join(".skill-lock.json");
    let mut lock: SkillLock = std::fs::read_to_string(&lock_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let rec = SkillRecord {
        name: BUILTIN_SKILL_NAME.into(),
        version: env!("APP_VERSION").into(),
        installed_at: now,
        source: "builtin".into(),
    };
    match lock.skills.iter().position(|r| r.name == rec.name) {
        Some(i) => lock.skills[i] = rec,
        None => lock.skills.push(rec),
    }
    let text = serde_json::to_string_pretty(&lock).map_err(|e| e.to_string())?;
    std::fs::write(&lock_path, text).map_err(|e| format!("写入锁清单失败：{e}"))?;
    Ok(skill_dir)
}

/// 已安装 skill 列表（读锁清单）。
#[cfg_attr(not(test), allow(dead_code))] // v1 由测试与 API 层预留，后续版本接 UI
pub fn installed(target_dir: Option<&Path>) -> Vec<SkillRecord> {
    let dir = match target_dir
        .map(Path::to_path_buf)
        .or_else(|| default_skill_dir().ok())
    {
        Some(d) => d,
        None => return vec![],
    };
    std::fs::read_to_string(dir.join(".skill-lock.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .map(|l: SkillLock| l.skills)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_frontmatter_parses() {
        let fields =
            parse_frontmatter(BUILTIN_SKILL_MD).expect("内置 skill frontmatter 必须可解析");
        assert!(fields
            .iter()
            .any(|(k, v)| k == "name" && v == BUILTIN_SKILL_NAME));
        assert!(fields.iter().any(|(k, _)| k == "description"));
    }

    #[test]
    fn frontmatter_bad_input() {
        assert!(parse_frontmatter("no frontmatter").is_err());
        assert!(parse_frontmatter("---\nbroken\n").is_err());
        assert!(parse_frontmatter("---\nname: x\n---\nbody").is_ok());
    }

    #[test]
    fn install_idempotent_and_locked() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = install_builtin(Some(dir.path())).unwrap();
        assert!(p1.join("SKILL.md").is_file());
        let p2 = install_builtin(Some(dir.path())).unwrap();
        assert_eq!(p1, p2);
        let list = installed(Some(dir.path()));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, BUILTIN_SKILL_NAME);
        assert_eq!(list[0].source, "builtin");
    }

    #[test]
    fn reject_executables() {
        let dir = tempfile::tempdir().unwrap();
        let s = dir.path().join("evil");
        std::fs::create_dir_all(&s).unwrap();
        std::fs::write(s.join("SKILL.md"), "---\nname: evil\n---\nx").unwrap();
        std::fs::write(s.join("run.sh"), "#!/bin/sh").unwrap();
        assert!(validate_skill_dir(&s).is_err());
    }
}
