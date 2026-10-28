//! 日志索引、过滤、分页与高亮区间（FR-LOG-03~06，NFR-PERF-02/03，ADR-05）。
//!
//! 文件源不整读内存：只缓存每行的字节偏移（10 万行 ≈ 0.8MB），取页时 seek 读。
//! 过滤流式扫描，仅保留命中行号。本模块 API 是同步阻塞的——
//! UI 侧用 `spawn_blocking`、API 侧在阻塞线程池调用。

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;

use crate::core::config::LogSource;
use crate::core::log_parser::{parse_line, Level, LogFormat, LogLine};

/// 过滤条件（多维度 AND，FR-LOG-03）。
#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    pub levels: HashSet<Level>,
    /// 标签子串
    pub tag: Option<String>,
    pub ts_from: Option<i64>,
    pub ts_to: Option<i64>,
    /// 消息体子串
    pub query: Option<String>,
}

impl LogFilter {
    fn matches(&self, line: &LogLine) -> bool {
        if !self.levels.is_empty()
            && line
                .level
                .map(|l| !self.levels.contains(&l))
                .unwrap_or(true)
        {
            return false;
        }
        if let Some(tag) = &self.tag {
            let t = tag.to_lowercase();
            if !line.tags.iter().any(|x| x.to_lowercase().contains(&t)) {
                return false;
            }
        }
        if let Some(from) = self.ts_from {
            if line.ts.map(|t| t < from).unwrap_or(true) {
                return false;
            }
        }
        if let Some(to) = self.ts_to {
            if line.ts.map(|t| t > to).unwrap_or(true) {
                return false;
            }
        }
        if let Some(q) = &self.query {
            if !line.message.to_lowercase().contains(&q.to_lowercase()) {
                return false;
            }
        }
        true
    }
}

/// 一页日志（FR-LOG-05/06）：行带绝对行号（供 UI 键控与展示）。
#[derive(Debug, Clone)]
pub struct LogPage {
    pub total_lines: usize,
    pub total_hits: usize,
    pub page: usize,
    pub page_size: usize,
    pub lines: Vec<(usize, LogLine)>,
}

enum Backend {
    File { path: PathBuf, offsets: Vec<u64> },
    Command { lines: Vec<String> },
}

/// 一个日志源的索引。
pub struct LogIndex {
    backend: Backend,
    format: LogFormat,
}

/// 命令模式单次输出的行数上限（ADR-05）。
pub const COMMAND_MAX_LINES: usize = 50_000;

impl LogIndex {
    /// 打开/构建索引。阻塞操作，调用方须在后台线程调用。
    pub fn open(src: &LogSource) -> Result<Self, String> {
        let format = LogFormat::parse(&src.format);
        if src.encoding != "utf-8" {
            return Err(format!("v1 仅支持 UTF-8 编码（当前：{}）", src.encoding));
        }
        if let Some(path) = src.path.as_deref().filter(|s| !s.is_empty()) {
            let path = PathBuf::from(path);
            let offsets = scan_offsets(&path)?;
            Ok(Self {
                backend: Backend::File { path, offsets },
                format,
            })
        } else if let Some(cmd) = src.command.as_deref().filter(|s| !s.is_empty()) {
            let lines = run_command_lines(cmd)?;
            Ok(Self {
                backend: Backend::Command { lines },
                format,
            })
        } else {
            Err("日志源未配置文件路径或读取命令".into())
        }
    }

    pub fn total_lines(&self) -> usize {
        match &self.backend {
            Backend::File { offsets, .. } => offsets.len(),
            Backend::Command { lines } => lines.len(),
        }
    }

    /// 读第 idx 行原始文本。
    fn raw_line(&self, idx: usize) -> Option<String> {
        match &self.backend {
            Backend::Command { lines } => lines.get(idx).cloned(),
            Backend::File { path, offsets } => {
                if idx >= offsets.len() {
                    return None;
                }
                let start = offsets[idx];
                let end = offsets.get(idx + 1).copied().unwrap_or(u64::MAX);
                let mut f = File::open(path).ok()?;
                f.seek(SeekFrom::Start(start)).ok()?;
                let mut buf = Vec::new();
                if end == u64::MAX {
                    f.read_to_end(&mut buf).ok()?;
                } else {
                    let mut take = f.take(end - start);
                    take.read_to_end(&mut buf).ok()?;
                }
                Some(
                    String::from_utf8_lossy(&buf)
                        .trim_end_matches(['\r', '\n'])
                        .to_string(),
                )
            }
        }
    }

    /// 过滤：返回命中行号（升序）与命中行上的标签集合（供 UI 快速选择，FR-LOG-03）。
    pub fn filter(&self, f: &LogFilter) -> (Vec<usize>, Vec<String>) {
        let mut hits = Vec::new();
        let mut tags: Vec<String> = Vec::new();
        let total = self.total_lines();
        for i in 0..total {
            let raw = match self.raw_line(i) {
                Some(r) => r,
                None => break,
            };
            let line = parse_line(self.format, &raw);
            if f.matches(&line) {
                for t in &line.tags {
                    if !tags.iter().any(|x| x == t) && tags.len() < 200 {
                        tags.push(t.clone());
                    }
                }
                hits.push(i);
            }
        }
        (hits, tags)
    }

    /// 取一页（在命中集合上分页）。
    pub fn page(&self, hits: &[usize], page: usize, page_size: usize) -> LogPage {
        let page_size = page_size.clamp(1, 10_000);
        let start = page.saturating_mul(page_size);
        let lines = hits
            .iter()
            .skip(start)
            .take(page_size)
            .filter_map(|&i| {
                self.raw_line(i)
                    .map(|raw| (i, parse_line(self.format, &raw)))
            })
            .collect();
        LogPage {
            total_lines: self.total_lines(),
            total_hits: hits.len(),
            page,
            page_size,
            lines,
        }
    }
}

/// 扫描文件构建行偏移索引。
fn scan_offsets(path: &PathBuf) -> Result<Vec<u64>, String> {
    let f = File::open(path).map_err(|e| format!("打开日志失败：{e}"))?;
    let mut reader = BufReader::new(f);
    let mut offsets = Vec::new();
    let mut offset = 0u64;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let n = reader
            .read_until(b'\n', &mut buf)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        offsets.push(offset);
        offset += n as u64;
    }
    Ok(offsets)
}

/// 执行读取命令，按行收集输出（限量，ADR-05）。
fn run_command_lines(cmd: &str) -> Result<Vec<String>, String> {
    use std::process::{Command, Stdio};
    let parts = crate::platform::commands::split_args(cmd);
    let Some((program, args)) = parts.split_first() else {
        return Err("空命令".into());
    };
    let out = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("执行读取命令失败：{e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if lines.len() > COMMAND_MAX_LINES {
        let drop = lines.len() - COMMAND_MAX_LINES;
        lines.drain(..drop);
    }
    Ok(lines)
}

/// 高亮区间：文本中 query 命中的字符区间（左闭右开，忽略大小写）。
/// 数据层预计算，调用方构造 rich span（R-02）。
pub fn highlight_ranges(text: &str, query: &str) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return vec![];
    }
    let chars: Vec<char> = text.chars().collect();
    let q: Vec<char> = query.chars().collect();
    if q.is_empty() || q.len() > chars.len() {
        return vec![];
    }
    let lower: Vec<char> = chars
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let ql: Vec<char> = q
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + ql.len() <= lower.len() {
        if lower[i..i + ql.len()] == ql[..] {
            out.push((i, i + ql.len()));
            i += ql.len();
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn write_log(dir: &Path, name: &str, content: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    fn src_with_path(p: &Path) -> LogSource {
        LogSource {
            name: "t".into(),
            path: Some(p.to_string_lossy().into_owned()),
            command: None,
            encoding: "utf-8".into(),
            format: "plain".into(),
        }
    }

    #[test]
    fn file_paging_and_filter() {
        let dir = tempfile::tempdir().unwrap();
        let mut content = String::new();
        for i in 0..250 {
            content.push_str(&format!("line {i} INFO ok\n"));
        }
        content.push_str("special ERROR bad thing\n");
        let p = write_log(dir.path(), "a.log", &content);
        let idx = LogIndex::open(&src_with_path(&p)).unwrap();
        assert_eq!(idx.total_lines(), 251);

        let (hits, _) = idx.filter(&LogFilter {
            query: Some("special".into()),
            ..Default::default()
        });
        assert_eq!(hits, vec![250]);
        let pg = idx.page(&hits, 0, 100);
        assert_eq!(pg.lines.len(), 1);
        assert_eq!(pg.total_hits, 1);
        assert_eq!(pg.lines[0].1.level, Some(Level::Error));

        // 分页：全量命中
        let (all, _) = idx.filter(&LogFilter::default());
        assert_eq!(all.len(), 251);
        let p0 = idx.page(&all, 0, 100);
        let p2 = idx.page(&all, 2, 100);
        assert_eq!(p0.lines.len(), 100);
        assert_eq!(p2.lines.len(), 51);
        assert!(p2.lines[50].1.raw.contains("special"));
        assert_eq!(p2.lines[50].0, 250);
    }

    #[test]
    fn level_and_time_filter() {
        let dir = tempfile::tempdir().unwrap();
        let p = write_log(
            dir.path(),
            "mix.log",
            "[ERROR] [db] 2024-05-06 07:08:09 down\n[INFO] [http] 2024-05-06 07:08:10 up\n",
        );
        let mut src = src_with_path(&p);
        src.format = "bracket".into();
        let idx = LogIndex::open(&src).unwrap();
        let f = LogFilter {
            levels: HashSet::from([Level::Error]),
            ..Default::default()
        };
        let (hits, _) = idx.filter(&f);
        assert_eq!(hits.len(), 1);
        let time_f = LogFilter {
            ts_from: crate::core::log_parser::parse_ts("2024-05-06 07:08:10"),
            ts_to: Some(i64::MAX),
            ..Default::default()
        };
        let (hits2, _) = idx.filter(&time_f);
        assert_eq!(hits2.len(), 1);
        // 标签提取
        let (_, tags) = idx.filter(&LogFilter::default());
        assert!(tags.contains(&"db".to_string()) && tags.contains(&"http".to_string()));
    }

    #[test]
    fn command_source() {
        let src = LogSource {
            name: "c".into(),
            path: None,
            command: Some(
                #[cfg(unix)]
                "printf 'x\\ny\\nz\\n'".into(),
                #[cfg(windows)]
                "cmd /C echo x".into(),
            ),
            encoding: "utf-8".into(),
            format: "plain".into(),
        };
        let idx = LogIndex::open(&src).unwrap();
        assert!(idx.total_lines() >= 1);
    }

    #[test]
    fn highlight_char_ranges() {
        assert_eq!(highlight_ranges("hello world", "world"), vec![(6, 11)]);
        assert_eq!(highlight_ranges("aA bB aA", "aa"), vec![(0, 2), (6, 8)]);
        assert_eq!(highlight_ranges("abc", ""), Vec::<(usize, usize)>::new());
        assert_eq!(highlight_ranges("短文本", "文"), vec![(1, 2)]);
    }

    #[test]
    fn gbk_rejected() {
        let src = LogSource {
            encoding: "gbk".into(),
            ..src_with_path(Path::new("/dev/null"))
        };
        assert!(LogIndex::open(&src).is_err());
    }
}
