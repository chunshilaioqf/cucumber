//! 系统字体枚举（任务 2）：Windows 注册表 / Linux fc-list / macOS 字体目录扫描。

/// 收集本机字体族名（去重排序；后台线程调用，阻塞 IO）。
pub fn list_families() -> Vec<String> {
    let mut set = std::collections::BTreeSet::new();
    match std::env::consts::OS {
        "windows" => collect_windows(&mut set),
        "macos" => collect_macos(&mut set),
        _ => collect_fontconfig(&mut set),
    }
    for f in ["monospace", "sans-serif", "serif"] {
        set.insert(f.to_string());
    }
    set.into_iter().collect()
}

fn collect_fontconfig(set: &mut std::collections::BTreeSet<String>) {
    if let Ok(out) = std::process::Command::new("fc-list")
        .args(["--format", "%{family[0]}\\n"])
        .output()
    {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let f = line.trim();
                if !f.is_empty() {
                    set.insert(f.to_string());
                }
            }
        }
    }
}

fn collect_windows(set: &mut std::collections::BTreeSet<String>) {
    for root in [
        "HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts",
        "HKCU\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts",
    ] {
        if let Ok(out) = std::process::Command::new("reg")
            .args(["query", root])
            .output()
        {
            if !out.status.success() {
                continue;
            }
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if let Some((_, v)) = line.split_once("REG_SZ") {
                    let name = v.trim().trim_end_matches("(TrueType)").trim();
                    let main = name.split('&').next().unwrap_or(name).trim();
                    if !main.is_empty() {
                        set.insert(main.to_string());
                    }
                }
            }
        }
    }
}

fn collect_macos(set: &mut std::collections::BTreeSet<String>) {
    let home = std::env::var("HOME").unwrap_or_default();
    let dirs = [
        "/System/Library/Fonts".to_string(),
        "/Library/Fonts".to_string(),
        format!("{home}/Library/Fonts"),
    ];
    for d in dirs {
        if let Ok(entries) = std::fs::read_dir(&d) {
            for e in entries.flatten() {
                let p = e.path();
                let ext = p
                    .extension()
                    .and_then(|x| x.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        let name = stem.split('-').next().unwrap_or(stem).trim().to_string();
                        if !name.is_empty() && name.chars().count() > 1 {
                            set.insert(name);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn families_never_empty() {
        let f = super::list_families();
        assert!(!f.is_empty());
        assert!(f.iter().any(|x| x == "monospace"));
    }
}
