//! sysinfo 封装（FR-PROC-08/13，ADR-06）：系统资源与进程观察。
//!
//! 采集在后台线程进行（每次全量 refresh + 构建快照），UI 只读快照。

use sysinfo::{ProcessesToUpdate, System};

/// 单个系统进程的观察数据。
#[derive(Debug, Clone)]
pub struct ProcSnapshot {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub mem_bytes: u64,
}

/// 一次采集的全量快照。
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub cpu_percent: f32,
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
    pub procs: Vec<ProcSnapshot>,
}

pub struct SysObserver {
    sys: System,
}

impl Default for SysObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl SysObserver {
    pub fn new() -> Self {
        let mut sys = System::new();
        // 首次刷新拿到基线，下一次才能算出 CPU 使用率
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        Self { sys }
    }

    /// 刷新并产出快照。调用间隔需 ≥ 200ms（sysinfo CPU 采样下限）。
    pub fn refresh(&mut self) -> Snapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);

        let cpus = self.sys.cpus();
        let cpu_percent = if cpus.is_empty() {
            0.0
        } else {
            cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32
        };

        let mut procs: Vec<ProcSnapshot> = self
            .sys
            .processes()
            .iter()
            .map(|(pid, p)| ProcSnapshot {
                pid: pid.as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                cpu_percent: p.cpu_usage(),
                mem_bytes: p.memory(),
            })
            .collect();
        procs.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));

        Snapshot {
            cpu_percent,
            mem_used_bytes: self.sys.used_memory(),
            mem_total_bytes: self.sys.total_memory(),
            procs,
        }
    }
}

/// 字节数的人类可读格式（UI/API 共用）。
pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        let unit = UNITS[i];
        format!("{v:.1} {unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_has_self_and_resources() {
        let mut obs = SysObserver::new();
        std::thread::sleep(std::time::Duration::from_millis(250));
        let s = obs.refresh();
        assert!(s.cpu_percent >= 0.0 && s.cpu_percent <= 100.0);
        assert!(s.mem_total_bytes > 0);
        // 至少能看到自己（测试进程）
        assert!(s.procs.iter().any(|p| p.name.contains("cucumber")));
    }

    #[test]
    fn human_bytes_fmt() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MB");
    }
}
