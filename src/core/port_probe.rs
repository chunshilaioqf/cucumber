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
