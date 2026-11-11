//! 硬件资源采集统一入口（0.6.0，ADR-21）：
//! [`all-smi`] 库为主数据源（NVIDIA/AMD/Apple Silicon/Intel + NPU，全平台），
//! 内置三路（macOS ioreg / Windows WMI / Linux sysfs）作回退——库初始化失败或
//! 无数据时仍可用。`AllSmi::new()` 在部分平台较慢（Apple IOReport 枚举），
//! 经 `OnceLock` 全局初始化一次，采集线程每周期仅做查询。

use std::sync::OnceLock;

/// GPU 摘要（多卡取第一块；count 记录卡数）。
#[derive(Debug, Clone, Default)]
pub struct GpuStat {
    pub utilization: f32,
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub temp_c: Option<u32>,
    pub power_w: Option<f64>,
    pub name: String,
    pub count: usize,
}

/// CPU 摘要（当前仅取温度增强；利用率仍由 sysinfo 提供以保历史口径一致）。
#[derive(Debug, Clone, Default)]
pub struct CpuStat {
    pub temp_c: Option<u32>,
}

static SMI: OnceLock<Option<all_smi::AllSmi>> = OnceLock::new();

fn smi() -> Option<&'static all_smi::AllSmi> {
    SMI.get_or_init(|| all_smi::AllSmi::new().ok()).as_ref()
}

/// GPU 摘要：all-smi 优先，无数据回退内置采集（仅利用率）。
pub fn gpu() -> Option<GpuStat> {
    if let Some(s) = smi() {
        let gpus = s.get_gpu_info();
        if let Some(g) = gpus.first() {
            // 个别后端无数据时利用率为负哨兵，视为无效
            let util = g.utilization;
            if util >= 0.0 {
                return Some(GpuStat {
                    utilization: util.clamp(0.0, 100.0) as f32,
                    used_bytes: g.used_memory,
                    total_bytes: g.total_memory,
                    temp_c: (g.temperature > 0).then_some(g.temperature),
                    power_w: (g.power_consumption >= 0.0).then_some(g.power_consumption),
                    name: g.name.clone(),
                    count: gpus.len(),
                });
            }
        }
    }
    // 回退：内置三路（仅利用率）
    crate::core::gpu::gpu_percent().map(|u| GpuStat {
        utilization: u,
        ..Default::default()
    })
}

/// CPU 增强指标（温度）。
pub fn cpu() -> CpuStat {
    let mut st = CpuStat::default();
    if let Some(s) = smi() {
        for c in s.get_cpu_info() {
            if let Some(t) = c.temperature {
                if t > 0 {
                    st.temp_c = Some(t);
                    break;
                }
            }
        }
    }
    st
}

#[cfg(test)]
mod tests {
    #[test]
    fn gpu_stat_is_well_formed_when_present() {
        let g = super::gpu();
        if let Some(g) = g {
            assert!((0.0..=100.0).contains(&g.utilization));
        }
        // CI 无 GPU：允许 None（回退路径也可能 None）
    }
}
