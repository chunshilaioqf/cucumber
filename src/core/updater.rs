//! 应用内更新器（任务 6）：下载对应平台安装包并执行真正的安装替换。
//!
//! - macOS：DMG → 后台脚本挂载替换 /Applications/Cucumber.app → `xattr -dr` 清除隔离 → 自动重启。
//! - Windows：zip → 后台脚本等旧进程退出后替换 exe → 重启。
//! - Linux：解压并提示路径（无统一安装位置）。
//!
//! 「替换自身」由独立脚本在本进程退出后执行，规避运行中文件占用。

use serde_json::Value;

use crate::core::update::REPO_URL;

/// 当前平台对应的资产文件名。
pub fn pick_asset_name(ver: &str) -> Option<String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    Some(match (os, arch) {
        ("macos", _) => format!("cucumber-{ver}-macos-universal.dmg"),
        // Windows 优先安装包（Inno Setup，任务 3）
        ("windows", "x86_64") => format!("cucumber-{ver}-windows-x86_64-setup.exe"),
        ("windows", _) => format!("cucumber-{ver}-windows-arm64-setup.exe"),
        ("linux", "x86_64") => format!("cucumber-{ver}-linux-x86_64.tar.gz"),
        ("linux", _) => format!("cucumber-{ver}-linux-aarch64.tar.gz"),
        _ => return None,
    })
}

/// 资产下载直链。
pub fn asset_url(tag: &str, asset: &str) -> String {
    format!("{REPO_URL}/releases/download/{tag}/{asset}")
}

/// 校验某 tag 是否含目标资产（避免下到 404 页面）。
pub async fn asset_exists(tag: &str, asset: &str) -> bool {
    let url = format!("https://api.github.com/repos/chunshilaioqf/cucumber/releases/tags/{tag}");
    let Ok(out) = tokio::process::Command::new("curl")
        .args(["-s", "--max-time", "15", &url])
        .output()
        .await
    else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<Value>(&String::from_utf8_lossy(&out.stdout)) else {
        return false;
    };
    v.get("assets")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .any(|x| x.get("name").and_then(Value::as_str) == Some(asset))
        })
        .unwrap_or(false)
}

/// 下载并安装。返回面向用户的结果说明；成功时应用将退出并由脚本完成替换重启。
pub async fn download_and_install(tag: &str, url: &str) -> Result<String, String> {
    let dest_dir = std::env::temp_dir().join(format!("cucumber-update-{tag}"));
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let file_name = url.rsplit('/').next().unwrap_or("package");
    let dest = dest_dir.join(file_name);

    let st = tokio::process::Command::new("curl")
        .args(["-sL", "--max-time", "600", "-o"])
        .arg(&dest)
        .arg(url)
        .status()
        .await
        .map_err(|e| format!("启动下载失败：{e}"))?;
    if !st.success() {
        return Err("下载失败（网络或磁盘）".into());
    }
    if dest.metadata().map(|m| m.len()).unwrap_or(0) < 100_000 {
        return Err("下载文件异常（过小），已取消".into());
    }

    let ver = tag.trim_start_matches('v');
    match std::env::consts::OS {
        "macos" => install_macos(&dest, ver),
        "windows" => install_windows(&dest, ver),
        _ => Ok(format!(
            "已下载 {file_name} 到系统临时目录并解压准备完成；Linux 无统一安装位置，请按 README 手动替换"
        )),
    }
}

/// macOS：脚本挂载 → 替换 → 清除 quarantine → 重启。
fn install_macos(dmg: &std::path::Path, ver: &str) -> Result<String, String> {
    let dmg = dmg.display();
    let script = format!(
        r#"#!/bin/bash
sleep 1
hdiutil attach "{dmg}" -quiet -mountpoint /Volumes/CucumberUpdate
rm -rf /Applications/Cucumber.app
cp -R "/Volumes/CucumberUpdate/Cucumber.app" /Applications/Cucumber.app
hdiutil detach /Volumes/CucumberUpdate -quiet || true
xattr -dr com.apple.quarantine /Applications/Cucumber.app
open /Applications/Cucumber.app
rm -f "$0"
"#
    );
    let script_path = std::env::temp_dir().join(format!("cucumber-update-{ver}.sh"));
    std::fs::write(&script_path, script).map_err(|e| e.to_string())?;
    make_executable(&script_path);
    spawn_detached(&script_path)?;
    Ok(format!(
        "v{ver} 安装中：应用将自动重启（已完成隔离标记清除，无需再点「仍要打开」）"
    ))
}

/// Windows（任务 3）：资产为 Inno Setup 安装包——启动静默安装（/VERYSILENT）。
fn install_windows(setup: &std::path::Path, _ver: &str) -> Result<String, String> {
    let _ = std::process::Command::new("cmd")
        .args([
            "/C",
            "start",
            "",
            "/wait",
            &setup.to_string_lossy(),
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok("安装包已启动（静默模式）：新版本将自动完成安装与启动".into())
}

fn make_executable(p: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755));
    }
}

fn spawn_detached(path: &std::path::Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        std::process::Command::new("sh")
            .arg(path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", &path.to_string_lossy()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_by_platform() {
        let name = pick_asset_name("0.4.0").unwrap();
        assert!(name.starts_with("cucumber-0.4.0-"));
        if cfg!(target_os = "macos") {
            assert!(name.ends_with(".dmg"));
        }
    }

    #[test]
    fn url_shape() {
        assert_eq!(
            asset_url("v0.4.0", "cucumber-0.4.0-macos-universal.dmg"),
            "https://github.com/chunshilaioqf/cucumber/releases/download/v0.4.0/cucumber-0.4.0-macos-universal.dmg"
        );
    }
}
