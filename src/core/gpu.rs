//! GPU 使用率（跨平台尽力而为；ADR-16）：
//! - macOS：`ioreg` 性能统计（Intel / Apple Silicon 键名与刻度各异，逐一归一）
//! - Windows：WMI 性能计数器 `GPU Engine(*engtype_3D)\Utilization Percentage`
//!   （与任务管理器同一数据源，覆盖 NVIDIA/AMD/Intel 全厂商）
//! - Linux：sysfs `gpu_busy_percent`（AMD/Intel）或 `nvidia-smi`（NVIDIA）
//!
//! 开源库取舍：`nvml-wrapper` 等仅覆盖 NVIDIA 单厂商且引入运行时绑定；
//! 三平台全部采用操作系统原生数据源的零依赖方案。

/// 解析 ioreg 的 PerformanceStatistics 片段，提取 GPU 利用率（%）。
#[cfg(any(target_os = "macos", test))]
pub fn parse_ioreg_gpu(text: &str) -> Option<f32> {
    if let Some(v) = extract_percent(text, "Device Utilization %") {
        return Some(v);
    }
    for key in ["GPU Utilization %", "gpu-utilization", "Utilization %"] {
        if let Some(v) = extract_percent(text, key) {
            return Some(v);
        }
    }
    if let Some(raw) = extract_raw(text, "IOGPUUtilization") {
        if let Ok(f) = raw.trim().parse::<f32>() {
            return Some(normalize(f));
        }
        if let Some(hex) = raw
            .trim()
            .strip_prefix("0x")
            .or_else(|| raw.trim().strip_prefix("0X"))
        {
            if let Ok(n) = u64::from_str_radix(hex, 16) {
                return Some(normalize(n as f32));
            }
        }
    }
    None
}

/// 数值刻度不一致（0–100 或 0–10000），归一到 0–100。
#[cfg(any(target_os = "macos", test))]
fn normalize(v: f32) -> f32 {
    if v > 100.0 {
        (v / 100.0).min(100.0)
    } else {
        v.clamp(0.0, 100.0)
    }
}

#[cfg(any(target_os = "macos", test))]
fn extract_percent(text: &str, key: &str) -> Option<f32> {
    let idx = text.find(key)?;
    let rest = &text[idx + key.len()..];
    let rest = rest.trim_start_matches([' ', '=', '"']);
    let num: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    num.parse::<f32>().ok().map(normalize)
}

#[cfg(any(target_os = "macos", test))]
fn extract_raw(text: &str, key: &str) -> Option<String> {
    let idx = text.find(key)?;
    let rest = &text[idx + key.len()..];
    let rest = rest.trim_start_matches([' ', '=', '"']);
    Some(
        rest.chars()
            .take_while(|c| !c.is_whitespace() && !";}),".contains(*c))
            .collect(),
    )
}

/// Windows：汇总 GPU Engine 3D 利用率计数器（与任务管理器同一数据源）。
#[cfg(any(target_os = "windows", test))]
pub fn parse_gpu_counter(text: &str) -> Option<f32> {
    let mut sum = 0.0f32;
    let mut n = 0usize;
    for line in text.lines() {
        let t = line.trim();
        if let Ok(v) = t.parse::<f32>() {
            if v >= 0.0 {
                sum += v;
                n += 1;
            }
        }
    }
    if n == 0 {
        None
    } else {
        Some(sum.min(100.0))
    }
}

#[cfg(target_os = "windows")]
fn gpu_windows() -> Option<f32> {
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-Counter '\\GPU Engine(*engtype_3D)\\Utilization Percentage').CounterSamples | ForEach-Object { $_.CookedValue }",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_gpu_counter(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(target_os = "linux")]
fn gpu_linux() -> Option<f32> {
    // AMD/Intel：sysfs gpu_busy_percent
    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        for e in entries.flatten() {
            let p = e.path().join("device/gpu_busy_percent");
            if let Ok(txt) = std::fs::read_to_string(&p) {
                if let Ok(v) = txt.trim().parse::<f32>() {
                    return Some(v.clamp(0.0, 100.0));
                }
            }
        }
    }
    // NVIDIA：nvidia-smi（存在时）
    if let Ok(out) = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=utilization.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        if out.status.success() {
            if let Some(first) = String::from_utf8_lossy(&out.stdout).lines().next() {
                if let Ok(v) = first.trim().parse::<f32>() {
                    return Some(v.clamp(0.0, 100.0));
                }
            }
        }
    }
    None
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn macos_gpu() -> Option<f32> {
    for class in ["IOGPU", "IOAccelerator", "AGXAccelerator"] {
        if let Ok(out) = std::process::Command::new("ioreg")
            .args(["-r", "-c", class, "-d", "1"])
            .output()
        {
            if !out.status.success() {
                continue;
            }
            let text = String::from_utf8_lossy(&out.stdout).into_owned();
            if let Some(v) = parse_ioreg_gpu(&text) {
                return Some(v);
            }
        }
    }
    None
}

/// 采集本机 GPU 使用率（%）。阻塞（子进程/sysfs）；后台线程调用。
pub fn gpu_percent() -> Option<f32> {
    #[cfg(target_os = "windows")]
    {
        gpu_windows()
    }
    #[cfg(target_os = "linux")]
    {
        gpu_linux()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        macos_gpu()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_intel_key() {
        let s = r#""PerformanceStatistics" = {"Device Utilization %"=42,"RendererID":1}"#;
        assert_eq!(parse_ioreg_gpu(s), Some(42.0));
    }

    #[test]
    fn parses_apple_silicon_keys() {
        assert_eq!(parse_ioreg_gpu(r#"{"IOGPUUtilization"=73}"#), Some(73.0));
        let v = parse_ioreg_gpu(r#"{"IOGPUUtilization"=0x3800}"#).unwrap();
        assert!((0.0..=100.0).contains(&v));
    }

    #[test]
    fn normalizes_high_scale() {
        assert_eq!(parse_ioreg_gpu(r#"{"GPU Utilization %"=4200}"#), Some(42.0));
    }

    #[test]
    fn windows_counter_sum() {
        assert_eq!(parse_gpu_counter("30.5\n25.5\n"), Some(56.0));
        assert_eq!(parse_gpu_counter("80\n80\n80\n"), Some(100.0));
        assert_eq!(parse_gpu_counter(""), None);
        assert_eq!(parse_gpu_counter("garbage"), None);
    }

    #[test]
    fn none_when_absent() {
        assert_eq!(parse_ioreg_gpu(r#"{"something"=1}"#), None);
    }
}
