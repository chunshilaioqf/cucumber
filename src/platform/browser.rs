//! 调起系统默认浏览器（FR-PROC-03）：平台层差异收口，不引入额外 crate。

/// 用系统默认浏览器/关联程序打开 URL（仅 http/https）。
///
/// Windows: `cmd /C start`；macOS: `open`；Linux: `xdg-open`。
/// 返回 Err 时由调用方以 toast 提示用户。
pub async fn open_url(url: &str) -> Result<(), String> {
    // 仅允许 http/https，避免协议处理器滥用
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("仅支持 http/https 地址".into());
    }
    #[cfg(target_os = "windows")]
    let (program, args): (&str, Vec<String>) = (
        "cmd",
        vec!["/C".into(), "start".into(), "".into(), url.to_string()],
    );
    #[cfg(target_os = "macos")]
    let (program, args): (&str, Vec<String>) = ("open", vec![url.to_string()]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let (program, args): (&str, Vec<String>) = ("xdg-open", vec![url.to_string()]);

    let status = tokio::process::Command::new(program)
        .args(&args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map_err(|e| format!("无法启动 {program}：{e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} 返回非零退出码"))
    }
}
