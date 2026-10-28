//! 跨平台命令执行封装（FR-PROC-04/05/06，NFR-SEC-02）。
//!
//! 所有外部命令都在后台 tokio 运行时里执行，UI 线程绝不阻塞。

use std::collections::BTreeMap;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::platform::signals;

/// 命令输出（FR-PROC-06）：stdout/stderr 截断到 64KB，避免失控输出拖垮内存。
#[derive(Debug, Clone, Default)]
pub struct CommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn ok(&self) -> bool {
        self.exit_code == Some(0)
    }
    /// 供 UI/API 展示的摘要。
    pub fn summary(&self) -> String {
        match self.exit_code {
            Some(c) => format!(
                "退出码 {c}\nstdout:\n{}\nstderr:\n{}",
                truncate(&self.stdout, 4096),
                truncate(&self.stderr, 4096)
            ),
            None => format!("命令未能执行\nstderr:\n{}", truncate(&self.stderr, 4096)),
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut cut = max;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}…(已截断)", &s[..cut])
    }
}

/// 按行累积输出，达到字节上限后丢弃更早的行（环形语义，NFR-PERF-03）。
#[derive(Debug, Default)]
pub struct OutputRing {
    lines: std::collections::VecDeque<String>,
    bytes: usize,
    cap: usize,
}

impl OutputRing {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            ..Default::default()
        }
    }

    pub fn push(&mut self, line: &str) {
        self.bytes += line.len() + 1;
        self.lines.push_back(line.to_string());
        while self.bytes > self.cap {
            if let Some(front) = self.lines.pop_front() {
                self.bytes -= front.len() + 1;
            } else {
                break;
            }
        }
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.lines.iter().cloned().collect()
    }
}

/// Shell 风格切分（NFR-SEC-02：默认参数数组，不穿 shell）：
/// 空白分隔；单/双引号包一段；反斜杠转义下一个字符（含空格与引号）。
pub fn split_args(cmd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut has = false;
    let mut chars = cmd.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                // 转义：保留下一个字符的字面量
                if let Some(next) = chars.next() {
                    cur.push(next);
                    has = true;
                }
            }
            '\'' | '"' => {
                has = true;
                for c in chars.by_ref() {
                    if c == ch {
                        break;
                    }
                    cur.push(c);
                }
            }
            c if c.is_whitespace() => {
                if has || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has || !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn build(
    program: &str,
    args: &[String],
    work_dir: Option<&str>,
    env: &BTreeMap<String, String>,
) -> Command {
    let mut c = Command::new(program);
    c.args(args);
    if let Some(dir) = work_dir.filter(|s| !s.is_empty()) {
        c.current_dir(dir);
    }
    for (k, v) in env {
        c.env(k, v);
    }
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW：GUI 子进程不闪黑框
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c
}

/// 执行并等待退出（stop/check/install_deps 语义）。timeout_secs 为总超时（小时级上限）。
pub async fn run_wait(
    cmd_line: &str,
    work_dir: Option<&str>,
    env: &BTreeMap<String, String>,
    timeout_secs: u64,
) -> CommandOutput {
    let parts = split_args(cmd_line);
    let Some((program, args)) = parts.split_first() else {
        return CommandOutput {
            exit_code: None,
            stdout: String::new(),
            stderr: "空命令".into(),
        };
    };
    let child = match build(program, args, work_dir, env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return CommandOutput {
                exit_code: None,
                stdout: String::new(),
                stderr: format!("启动失败：{e}"),
            }
        }
    };
    let pid = child.id();
    let fut = child.wait_with_output();
    let out = match tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs.clamp(1, 86_400)),
        fut,
    )
    .await
    {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            return CommandOutput {
                exit_code: None,
                stdout: String::new(),
                stderr: format!("等待失败：{e}"),
            }
        }
        Err(_) => {
            // 超时兜底：尽力杀掉进程树
            if let Some(p) = pid {
                signals::terminate_pid(p).await;
            }
            return CommandOutput {
                exit_code: None,
                stdout: String::new(),
                stderr: "执行超时".into(),
            };
        }
    };
    CommandOutput {
        exit_code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// spawn 长驻子进程（start_command 默认语义）：立即返回 PID，等待放后台任务。
/// 输出逐行泵入环形缓冲；子进程退出时在后台任务中调用 `on_exit(退出码)`。
pub async fn spawn_child(
    cmd_line: &str,
    work_dir: Option<&str>,
    env: &BTreeMap<String, String>,
    ring: std::sync::Arc<std::sync::Mutex<OutputRing>>,
    on_exit: Box<dyn FnOnce(Option<i32>) + Send + 'static>,
) -> Result<u32, String> {
    let parts = split_args(cmd_line);
    let Some((program, args)) = parts.split_first() else {
        return Err("空命令".into());
    };
    let mut child = build(program, args, work_dir, env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| e.to_string())?;
    let pid = child.id().unwrap_or(0);

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let mut pumps = Vec::new();
    if let Some(s) = stdout {
        pumps.push(spawn_pump(s, ring.clone()));
    }
    if let Some(s) = stderr {
        pumps.push(spawn_pump(s, ring.clone()));
    }

    // 后台等待：退出后收尾泵任务并通知回调
    tokio::spawn(async move {
        let status = child.wait().await;
        for p in pumps {
            let _ = p.await;
        }
        on_exit(status.ok().and_then(|s| s.code()));
    });
    Ok(pid)
}

/// 逐行泵送子进程输出到环形缓冲（stdout/stderr 各一份任务）。
fn spawn_pump<S>(
    s: S,
    ring: std::sync::Arc<std::sync::Mutex<OutputRing>>,
) -> tokio::task::JoinHandle<()>
where
    S: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut reader = BufReader::new(s);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf);
                    if let Ok(mut r) = ring.lock() {
                        r.push(line.trim_end_matches(['\r', '\n']));
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_basic() {
        assert_eq!(
            split_args("python3 -m http.server 8080"),
            ["python3", "-m", "http.server", "8080"]
        );
        assert_eq!(
            split_args("echo \"hello world\" 'a b'"),
            ["echo", "hello world", "a b"]
        );
        assert_eq!(split_args(""), Vec::<String>::new());
        assert_eq!(split_args("a\\ b"), ["a b"]);
    }

    #[tokio::test]
    async fn run_wait_success_and_failure() {
        let env = BTreeMap::new();
        let ok = run_wait("echo hello", None, &env, 0).await;
        assert_eq!(ok.exit_code, Some(0));
        assert!(ok.stdout.contains("hello"));

        #[cfg(unix)]
        let fail_cmd = "sh -c 'exit 3'";
        #[cfg(windows)]
        let fail_cmd = "cmd /C exit 3";
        let fail = run_wait(fail_cmd, None, &env, 0).await;
        assert_eq!(fail.exit_code, Some(3));
    }

    #[test]
    fn ring_eviction() {
        let mut r = OutputRing::new(16);
        for i in 0..10 {
            r.push(&format!("line-{i}-xxxxxxxx"));
        }
        let s = r.snapshot();
        assert!(s.len() < 10);
        assert!(s.last().unwrap().contains("line-9"));
    }
}
