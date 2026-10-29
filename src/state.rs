//! 全局应用状态：UI、后台采集与对外接口共享的汇合点。

use std::path::PathBuf;
use std::sync::atomic::AtomicU16;
use std::sync::{Arc, RwLock};

use crate::core::config::Config;
use crate::core::process_manager::ProcessManager;
use crate::core::sys_observer::Snapshot;

/// 进程/指标事件广播（WebSocket 推送用）。
pub type EventTx = tokio::sync::broadcast::Sender<String>;

pub struct AppState {
    pub config: RwLock<Config>,
    pub config_path: PathBuf,
    pub mgr: ProcessManager,
    /// 最近一次系统快照（采集线程写入）
    pub last_snapshot: RwLock<Option<Snapshot>>,
    /// 实际监听端口（绑定后写入；0 = 未启动）
    pub http_port: AtomicU16,
    pub ws_port: AtomicU16,
    /// WS 事件通道
    pub events: EventTx,
    /// 端口探测结果（proc_id → 是否连通；0.3.1，ADR-14）
    pub port_status: RwLock<std::collections::HashMap<String, bool>>,
}

impl AppState {
    pub fn new(config: Config, config_path: PathBuf) -> Arc<Self> {
        let (events, _) = tokio::sync::broadcast::channel(64);
        Arc::new(Self {
            config: RwLock::new(config),
            config_path,
            mgr: ProcessManager::new(),
            last_snapshot: RwLock::new(None),
            http_port: AtomicU16::new(0),
            ws_port: AtomicU16::new(0),
            events,
            port_status: RwLock::new(std::collections::HashMap::new()),
        })
    }

    pub fn snapshot(&self) -> Option<Snapshot> {
        self.last_snapshot.read().ok().and_then(|s| s.clone())
    }

    pub fn refresh_secs(&self) -> u64 {
        self.config
            .read()
            .map(|c| c.general.refresh_secs.max(1))
            .unwrap_or(3)
    }

    /// 持久化配置。
    pub fn save_config(&self) -> Result<(), String> {
        let cfg = self.config.read().map_err(|e| e.to_string())?;
        cfg.save(&self.config_path)
    }
}

pub type SharedState = Arc<AppState>;
