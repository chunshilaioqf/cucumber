//! GPU 使用率（任务 3）：macOS 经 `ioreg` 性能统计尽力解析；其他平台暂无数据。
//! 字段名随芯片代际不同（Intel "Device Utilization %" / Apple Silicon "IOGPUUtilization" 等），
//! 解析全部已知键，失败返回 None（帮助页注明限制）。

/// 解析 ioreg 的 PerformanceStatistics 片段，提取 GPU 利用率（%）。
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
fn normalize(v: f32) -> f32 {
    if v > 100.0 {
        (v / 100.0).min(100.0)
    } else {
        v.clamp(0.0, 100.0)
    }
}

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

fn extract_raw(text: &str, key: &str) -> Option<String> {
    let idx = text.find(key)?;
    let rest = &text[idx + key.len()..];
    let rest = rest.trim_start_matches([' ', '=', '"']);
    Some(
        rest.chars()
            .take_while(|c| !c.is_whitespace() && *c != ';' && *c != '}' && *c != ',' && *c != ')')
            .collect(),
    )
}

/// 采集本机 GPU 使用率（%）。阻塞（ioreg 子进程）；后台线程调用。
pub fn gpu_percent() -> Option<f32> {
    if cfg!(target_os = "macos") {
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
    } else {
        None
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
    fn none_when_absent() {
        assert_eq!(parse_ioreg_gpu(r#"{"something"=1}"#), None);
    }
}
