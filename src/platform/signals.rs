//! 停止信号（R-05 / ADR-04）：Unix 走 `kill`，Windows 走 `taskkill`（无 SIGTERM 对等物）。
//!
//! “尽力而为”：系统进程模式下工具不保证能停掉任意进程；
//! 配置进程的首选停止机制始终是 stop_command。

/// 优雅终止（SIGTERM / taskkill 不带 /F）。
pub async fn terminate_pid(pid: u32) -> bool {
    #[cfg(unix)]
    let line = format!("kill -TERM {pid}");
    #[cfg(windows)]
    let line = format!("taskkill /PID {pid} /T");
    run(&line).await
}

/// 强制终止（SIGKILL / taskkill /F）。
pub async fn kill_pid(pid: u32) -> bool {
    #[cfg(unix)]
    let line = format!("kill -KILL {pid}");
    #[cfg(windows)]
    let line = format!("taskkill /PID {pid} /T /F");
    run(&line).await
}

async fn run(line: &str) -> bool {
    let parts = crate::platform::commands::split_args(line);
    let Some((program, args)) = parts.split_first() else {
        return false;
    };
    tokio::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn terminate_nonexistent_is_false() {
        // PID 4_000_000 在常见系统上不存在；kill 返回非零即“尽力而为失败”
        assert!(!terminate_pid(4_000_000).await);
    }
}
