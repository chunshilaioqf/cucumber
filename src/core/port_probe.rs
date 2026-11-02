//! 服务端口探测（0.3.1，ADR-14）：TCP 连通性作为运行状态的第二真相源。
//!
//! 背景：外部启动（systemctl / 手动 / nohup / 工具重启后仍在跑的点火即走服务）
//! 无法被“本工具 spawn 的子进程存活”这一唯一判定看见。对配置了地址的
//! Web 服务周期性 TCP 探测，与状态机融合为「有效状态」。

use std::collections::HashMap;

/// 单地址 TCP 探测：可连通返回 true（超时/拒绝/无路由均 false）。
pub async fn tcp_up(host: &str, port: u16, timeout_ms: u64) -> bool {
    let addr = format!("{host}:{port}");
    let target: std::net::SocketAddr = match addr.parse() {
        Ok(a) => a,
        Err(_) => {
            // 主机名（非字面 IP）：先解析再连
            let Ok(mut addrs) = tokio::net::lookup_host(&addr).await else {
                return false;
            };
            let Some(a) = addrs.next() else { return false };
            a
        }
    };
    tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        tokio::net::TcpStream::connect(target),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
}

/// 探测一个进程的全部地址：任一 TCP/http/https 地址连通即为 up；udp 无法探测（忽略）。
pub async fn proc_port_up(addresses: &[crate::core::config::Address]) -> Option<bool> {
    let mut any = false;
    for a in addresses {
        match a.protocol.as_str() {
            "tcp" | "http" | "https" => {
                any = true;
                if tcp_up(&a.host, a.port, 800).await {
                    return Some(true);
                }
            }
            _ => {} // udp 无连接语义，跳过
        }
    }
    if any {
        Some(false)
    } else {
        None
    }
}

/// 批量探测：proc_id → 端口是否连通（未配置可探测地址的进程不在结果中）。
pub async fn probe_all(processes: &[crate::core::config::ProcessConfig]) -> HashMap<String, bool> {
    let mut out = HashMap::new();
    for p in processes {
        if p.addresses.is_empty() {
            continue;
        }
        if let Some(up) = proc_port_up(&p.addresses).await {
            out.insert(p.id.clone(), up);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Address;

    #[tokio::test]
    async fn probe_listening_and_closed_ports() {
        // 起一个真实监听
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        let up = Address {
            protocol: "http".into(),
            host: "127.0.0.1".into(),
            port,
            path: None,
        };
        assert_eq!(proc_port_up(&[up]).await, Some(true));

        // 关闭的端口（1.1.1.1 保留段之外的本地高位未监听端口大概率拒绝）
        let closed = Address {
            protocol: "tcp".into(),
            host: "127.0.0.1".into(),
            port: 1,
            path: None,
        };
        assert_eq!(proc_port_up(&[closed]).await, Some(false));
    }

    #[tokio::test]
    async fn udp_addresses_are_skipped() {
        let udp = Address {
            protocol: "udp".into(),
            host: "127.0.0.1".into(),
            port: 53,
            path: None,
        };
        assert_eq!(proc_port_up(&[udp]).await, None);
    }

    #[tokio::test]
    async fn timeout_does_not_hang() {
        // 不可路由地址走超时而非挂死（10.255.255.1:9 常见黑洞）
        let t0 = std::time::Instant::now();
        let _ = tcp_up("10.255.255.1", 9, 300).await; // 结果不定，只验证限时返回
        assert!(t0.elapsed() < std::time::Duration::from_secs(3));
    }
}

// ---------- 端口 → PID 反查（0.5.0，ADR-20） ----------
// 外部启动的服务（端口探测判定运行中）同样显示真实 PID：
// Linux 走 /proc（零子进程）、macOS 走 lsof、Windows 走 netstat（CREATE_NO_WINDOW）。
// 多个监听进程（nginx master/worker、SO_REUSEPORT）取第一个。

/// 返回监听该端口的 PID（取第一个）。找不到返回 None。
pub fn pid_listening_on(port: u16) -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        pid_from_proc(port)
    }
    #[cfg(target_os = "macos")]
    {
        pid_from_lsof(port)
    }
    #[cfg(target_os = "windows")]
    {
        pid_from_netstat(port)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = port;
        None
    }
}

/// Linux：/proc/net/tcp{,6} 的 LISTEN 行 → inode → /proc/<pid>/fd 匹配 socket:[inode]。
#[cfg(target_os = "linux")]
fn pid_from_proc(port: u16) -> Option<u32> {
    let hex_port = format!("{:04X}", port);
    let mut inodes: Vec<String> = Vec::new();
    for f in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        for line in text.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            // cols: sl local_address rem_address st ...
            if cols.len() > 9 {
                let local = cols[1];
                let st = cols[3];
                let (Some(lp), ok) = (local.rsplit(':').next(), st == "0A") else {
                    continue;
                };
                if ok && lp.eq_ignore_ascii_case(&hex_port) {
                    inodes.push(cols[9].to_string());
                }
            }
        }
    }
    if inodes.is_empty() {
        return None;
    }
    // 遍历 /proc/<pid>/fd：socket:[inode]
    let mut best: Option<u32> = None;
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let fd_dir = entry.path().join("fd");
        let Ok(fds) = std::fs::read_dir(&fd_dir) else {
            continue;
        };
        for fd in fds.flatten() {
            if let Ok(target) = fd.path().read_link() {
                let t = target.to_string_lossy();
                if let Some(inode) = t.strip_prefix("socket:[") {
                    let inode = inode.trim_end_matches(']');
                    if inodes.iter().any(|i| i == inode) {
                        // 取数值最小的 PID（master 先于 worker 创建）或当前第一个
                        if best.is_none() || pid < best.unwrap_or(u32::MAX) {
                            best = Some(pid);
                        }
                        break;
                    }
                }
            }
        }
    }
    best
}

/// macOS：lsof -Fp（输出行 p<PID>；默认升序，取第一个）。
#[cfg(target_os = "macos")]
fn pid_from_lsof(port: u16) -> Option<u32> {
    let out = std::process::Command::new("lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-Fp"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix('p').and_then(|p| p.parse::<u32>().ok()))
}

/// Windows：netstat -ano（LISTENING 行末列为 PID，取第一个；CREATE_NO_WINDOW 防闪窗）。
#[cfg(target_os = "windows")]
fn pid_from_netstat(port: u16) -> Option<u32> {
    let mut cmd = std::process::Command::new("netstat");
    crate::platform::no_window::NoWindow::no_window(&mut cmd);
    let out = cmd.args(["-ano", "-p", "TCP"]).output().ok()?;
    let needle = format!(":{port}");
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if !line.contains("LISTENING") {
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        // local address 形如 0.0.0.0:8080 / [::]:8080
        if cols.len() >= 5 && cols[1].ends_with(&needle) {
            return cols[4].parse::<u32>().ok();
        }
    }
    None
}

#[cfg(test)]
mod fusion_tests {
    use crate::core::process_manager::{ProcStatus, State};

    #[test]
    fn display_pid_falls_back_to_port_pid() {
        let st = ProcStatus {
            port_up: Some(true),
            port_pid: Some(4321),
            ..Default::default()
        };
        assert_eq!(st.display_pid(), Some(4321)); // 外部启动：反查 PID
        let st2 = ProcStatus {
            pid: Some(111),
            port_pid: Some(4321),
            ..Default::default()
        };
        assert_eq!(st2.display_pid(), Some(111)); // 自有启动优先
        let st3 = ProcStatus {
            observe_pid: Some(222),
            port_pid: Some(4321),
            ..Default::default()
        };
        assert_eq!(st3.display_pid(), Some(222)); // 系统观察次之
    }

    #[test]
    fn externally_running_includes_pid() {
        let st = ProcStatus {
            state: Some(State::Stopped),
            port_up: Some(true),
            port_pid: Some(999),
            ..Default::default()
        };
        assert!(st.externally_running());
        assert_eq!(st.effective_state(), State::Running);
        assert_eq!(st.display_pid(), Some(999));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod pid_tests {
    use super::*;

    #[test]
    fn finds_listener_pid_on_linux() {
        // 真实监听 + 反查自身
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let me = std::process::id();
        let found = pid_listening_on(port);
        assert_eq!(found, Some(me), "应找到本测试进程的监听 PID");
    }

    #[test]
    fn none_for_unlistened_port() {
        // 取一个几乎不可能被监听的高位端口
        assert_eq!(pid_listening_on(59999), None);
    }
}
