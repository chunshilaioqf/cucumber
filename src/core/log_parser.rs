//! 日志行解析（FR-LOG-07，ADR-02）：JSON lines / logfmt / bracket / plain 四种预设，
//! 手写解析器（不引 regex/chrono，内存预算见 NFR-PERF-03）。

use serde_json::Value;

/// 日志级别（FR-LOG-03）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Level {
    pub fn label(&self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }

    pub fn parse(s: &str) -> Option<Level> {
        match s.trim().trim_end_matches(',').to_ascii_uppercase().as_str() {
            "ERROR" | "ERR" | "FATAL" | "CRITICAL" | "CRIT" => Some(Level::Error),
            "WARN" | "WARNING" => Some(Level::Warn),
            "INFO" | "NOTICE" => Some(Level::Info),
            "DEBUG" | "DBG" => Some(Level::Debug),
            "TRACE" => Some(Level::Trace),
            _ => None,
        }
    }

    pub fn all() -> [Level; 5] {
        [
            Level::Error,
            Level::Warn,
            Level::Info,
            Level::Debug,
            Level::Trace,
        ]
    }
}

/// 解析后的日志行（FR-LOG-07）。
#[derive(Debug, Clone, PartialEq)]
pub struct LogLine {
    pub raw: String,
    pub level: Option<Level>,
    pub tags: Vec<String>,
    /// Unix 秒
    pub ts: Option<i64>,
    pub message: String,
}

impl LogLine {
    fn plain(raw: String) -> Self {
        // 纯文本也做级别嗅探：独立的 ERROR/WARN/… 词（词边界判断，避免匹配 ERRONEOUS）
        let level = sniff_level(&raw);
        Self {
            raw,
            level,
            tags: vec![],
            ts: None,
            message: String::new(),
        }
    }
}

fn sniff_level(s: &str) -> Option<Level> {
    let up = s.to_ascii_uppercase();
    let mut rest = up.as_str();
    while let Some(pos) = rest.find(|c: char| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        rest = &rest[pos..];
        let mut end = 0;
        for (i, ch) in rest.char_indices() {
            if !(ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_') {
                end = i;
                break;
            }
            end = i + ch.len_utf8();
        }
        let word = &rest[..end];
        if let Some(l) = Level::parse(word) {
            return Some(l);
        }
        rest = &rest[end..];
    }
    None
}

/// 行格式标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    JsonLines,
    Logfmt,
    /// `[LEVEL] [TAG] timestamp message`
    Bracket,
    Plain,
}

impl LogFormat {
    pub fn parse(s: &str) -> Self {
        match s {
            "json_lines" | "json" => Self::JsonLines,
            "logfmt" => Self::Logfmt,
            "bracket" => Self::Bracket,
            _ => Self::Plain,
        }
    }
}

/// 解析一行。
pub fn parse_line(format: LogFormat, raw: &str) -> LogLine {
    let mut line = LogLine::plain(raw.to_string());
    match format {
        LogFormat::Plain => {}
        _ => {
            // 结构化格式不依赖嗅探：显式重置，由各分支自己填
            line.level = None;
        }
    }
    match format {
        LogFormat::Plain => {}
        LogFormat::JsonLines => {
            if let Ok(v) = serde_json::from_str::<Value>(raw) {
                if let Some(level) = v
                    .get("level")
                    .or_else(|| v.get("severity"))
                    .and_then(Value::as_str)
                    .and_then(Level::parse)
                {
                    line.level = Some(level);
                }
                line.ts = v
                    .get("timestamp")
                    .or_else(|| v.get("time"))
                    .or_else(|| v.get("ts"))
                    .and_then(|t| match t {
                        Value::Number(n) => n.as_i64().and_then(|x| parse_ts(&x.to_string())),
                        Value::String(s) => parse_ts(s),
                        _ => None,
                    });
                if let Some(msg) = v
                    .get("message")
                    .or_else(|| v.get("msg"))
                    .and_then(Value::as_str)
                {
                    line.message = msg.to_string();
                }
                match v.get("tag").or_else(|| v.get("tags")) {
                    Some(Value::String(s)) => line.tags.push(s.clone()),
                    Some(Value::Array(a)) => {
                        for t in a {
                            if let Some(s) = t.as_str() {
                                line.tags.push(s.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        LogFormat::Logfmt => {
            for (k, v) in parse_logfmt_pairs(raw) {
                match k.as_str() {
                    "level" | "severity" => line.level = Level::parse(&v),
                    "msg" | "message" => line.message = v,
                    "ts" | "time" | "timestamp" => line.ts = parse_ts(&v),
                    "tag" => line.tags.push(v),
                    "tags" => line.tags.extend(v.split(';').map(str::to_string)),
                    _ => {}
                }
            }
        }
        LogFormat::Bracket => {
            let mut rest = raw.trim_start();
            // 连续的 [X] 前缀段（段间可有空白）
            loop {
                rest = rest.trim_start();
                let Some(t) = rest.strip_prefix('[') else {
                    break;
                };
                let Some(end) = t.find(']') else { break };
                let seg = &t[..end];
                rest = &t[end + 1..];
                if line.level.is_none() {
                    if let Some(l) = Level::parse(seg) {
                        line.level = Some(l);
                        continue;
                    }
                }
                if line.ts.is_none() {
                    if let Some(ts) = parse_ts(seg) {
                        line.ts = Some(ts);
                        continue;
                    }
                }
                line.tags.push(seg.to_string());
            }
            if line.ts.is_none() {
                let trimmed = rest.trim_start();
                // 先试「日期 时间」两段组合，再退回单段（如纯日期或纯时间戳）
                let mut parts = trimmed.splitn(3, ' ');
                let (a, b) = (parts.next().unwrap_or(""), parts.next());
                let mut consumed_len = 0;
                let mut ts = b.and_then(|b| parse_ts(&format!("{a} {b}"))); // 完整「date time」
                if ts.is_none() {
                    ts = parse_ts(a);
                    if ts.is_some() {
                        consumed_len = a.len();
                    }
                } else {
                    consumed_len = a.len() + 1 + b.unwrap_or("").len();
                }
                if let Some(t) = ts {
                    line.ts = Some(t);
                    rest = &trimmed[consumed_len..];
                }
            }
            line.message = rest.trim_start().to_string();
        }
    }
    if line.message.is_empty() {
        line.message = line.raw.trim().to_string();
    }
    line
}

/// logfmt 键值对解析（k=v，空格分隔，双引号包值）。
fn parse_logfmt_pairs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let n = chars.len();
    while i < n {
        while i < n && chars[i] == ' ' {
            i += 1;
        }
        let start = i;
        while i < n && chars[i] != '=' && chars[i] != ' ' {
            i += 1;
        }
        if i >= n || chars[i] == ' ' {
            continue;
        }
        let key: String = chars[start..i].iter().collect();
        i += 1; // '='
        let mut val = String::new();
        if i < n && chars[i] == '"' {
            i += 1;
            while i < n && chars[i] != '"' {
                val.push(chars[i]);
                i += 1;
            }
            i += 1;
        } else {
            while i < n && chars[i] != ' ' {
                val.push(chars[i]);
                i += 1;
            }
        }
        if !key.is_empty() {
            out.push((key, val));
        }
    }
    out
}

/// 宽松时间戳解析：epoch 秒/毫秒、RFC3339、`YYYY-MM-DD HH:MM:SS`。
pub fn parse_ts(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<i64>() {
        return Some(if n > 10_000_000_000 { n / 1000 } else { n });
    }
    let b = s.as_bytes();
    let looks_like_date = b.len() >= 19
        && b[..4].iter().all(|c| c.is_ascii_digit())
        && b[4] == b'-'
        && b[7] == b'-'
        && (b[10] == b'T' || b[10] == b' ');
    if !looks_like_date {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let mo: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    let h: i64 = s.get(11..13)?.parse().ok()?;
    let mi: i64 = s.get(14..16)?.parse().ok()?;
    let sec: i64 = s.get(17..19)?.parse().ok()?;
    let mut secs = days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + sec;
    // 时区后缀：Z / +hh:mm / -hh:mm / +hhmm
    let tail = &s[19..];
    let tz = tail.split(['.', 'Z', 'z']).next().unwrap_or("");
    let tz_trim = tz.trim();
    if let Some(off) = tz_trim
        .strip_prefix('+')
        .or_else(|| tz_trim.strip_prefix('-'))
    {
        let sign = if tz_trim.starts_with('-') { -1 } else { 1 };
        let clean: String = off.chars().filter(|c| c.is_ascii_digit()).collect();
        if clean.len() >= 4 {
            let oh: i64 = clean.get(0..2)?.parse().ok()?;
            let om: i64 = clean.get(2..4)?.parse().ok()?;
            secs -= sign * (oh * 3600 + om * 60);
        } else if clean.len() >= 2 {
            let oh: i64 = clean.get(0..2)?.parse().ok()?;
            secs -= sign * oh * 3600;
        }
    }
    Some(secs)
}

/// Howard Hinnant 的 civil → days 算法（无外部依赖的日期计算）。
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Unix 秒 → 展示串 `YYYY-MM-DD HH:MM:SS`（简化：按 UTC 展示，日志场景够用）。
pub fn format_ts(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_lines() {
        let l = parse_line(
            LogFormat::JsonLines,
            r#"{"level":"ERROR","timestamp":"2024-05-06T07:08:09Z","message":"boom","tag":"db"}"#,
        );
        assert_eq!(l.level, Some(Level::Error));
        assert_eq!(l.tags, vec!["db".to_string()]);
        assert_eq!(l.message, "boom");
        assert!(l.ts.is_some());
        assert_eq!(format_ts(l.ts.unwrap()), "2024-05-06 07:08:09");
    }

    #[test]
    fn json_epoch_ms() {
        let l = parse_line(
            LogFormat::JsonLines,
            r#"{"level":"info","ts":1714974489000,"msg":"hi"}"#,
        );
        assert_eq!(l.level, Some(Level::Info));
        assert_eq!(l.ts, Some(1_714_974_489));
    }

    #[test]
    fn logfmt_pairs() {
        let l = parse_line(
            LogFormat::Logfmt,
            r#"level=warn msg="conn refused" ts=1714974489 tag=net"#,
        );
        assert_eq!(l.level, Some(Level::Warn));
        assert_eq!(l.message, "conn refused");
        assert_eq!(l.ts, Some(1_714_974_489));
        assert_eq!(l.tags, vec!["net".to_string()]);
    }

    #[test]
    fn bracket_format() {
        let l = parse_line(
            LogFormat::Bracket,
            "[INFO] [http] 2024-05-06 07:08:09 GET /health 200",
        );
        assert_eq!(l.level, Some(Level::Info));
        assert_eq!(l.tags, vec!["http".to_string()]);
        assert!(l.ts.is_some());
        assert!(l.message.starts_with("GET /health"));
    }

    #[test]
    fn plain_sniff() {
        assert_eq!(
            parse_line(LogFormat::Plain, "something ERROR here").level,
            Some(Level::Error)
        );
        assert_eq!(parse_line(LogFormat::Plain, "ERRONEOUS input").level, None);
        assert_eq!(
            parse_line(LogFormat::Plain, "2024-05-06 07:08:09 WARN low disk").level,
            Some(Level::Warn)
        );
    }

    #[test]
    fn ts_variants() {
        assert_eq!(parse_ts("1714974489"), Some(1_714_974_489));
        assert_eq!(parse_ts("1714974489123"), Some(1_714_974_489));
        assert_eq!(
            parse_ts("2024-05-06T07:08:09+08:00"),
            parse_ts("2024-05-06T07:08:09Z").map(|t| t - 8 * 3600)
        );
        assert_eq!(parse_ts("not-a-time"), None);
        assert_eq!(parse_ts(""), None);
    }

    #[test]
    fn ts_roundtrip() {
        for t in [0i64, 1_700_000_000, 1_714_974_489] {
            assert_eq!(parse_ts(&format_ts(t)), Some(t));
        }
    }
}
