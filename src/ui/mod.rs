//! UI 层：windui 页面与全局上下文（NFR-MAINT-03：经 `kit` 适配层收口框架调用）。

pub mod about;
pub mod api_page;
pub mod app;
pub mod dashboard;
pub mod docs;
pub mod kit;
pub mod logs;
pub mod processes;
pub mod services;

use windui::prelude::*;

use crate::core::config::{ProcessConfig, ProcessType};
use crate::core::process_manager::State;
use crate::core::sys_observer::human_bytes;

/// 后台 → UI 线程消息（ADR-06：Signal 只能在 UI 线程写）。
pub enum UiMsg {
    /// 采集线程完成一轮刷新：从 AppState 重算所有派生 Signal。
    Snapshot,
    ToastOk(String),
    ToastErr(String),
    /// 日志查询结果（行、命中数、摘要、页码）
    LogReady(Vec<UiLogLine>, usize, String, usize),
    /// 快速诊断结果（FR-HELP-02）
    HealthCheck(bool, String),
}

/// 进程列表行（UI 中间态）。
#[derive(Debug, Clone, PartialEq)]
pub struct ProcRow {
    pub id: String,
    pub name: String,
    pub type_label: String,
    pub state_label: String,
    pub pid_text: String,
    pub res_text: String,
    pub running: bool,
    pub observe_only: bool,
    pub has_start: bool,
    pub has_stop: bool,
    pub has_check: bool,
}

/// 服务列表行（FR-SERV-02）。
#[derive(Debug, Clone, PartialEq)]
pub struct SvcRow {
    pub url: String,
    pub proc_name: String,
    pub pid_text: String,
    pub openable: bool,
}

/// UI 渲染用日志行（绝对行号 + 解析结果）。
#[derive(Debug, Clone, PartialEq)]
pub struct UiLogLine {
    pub no: usize,
    pub line: crate::core::log_parser::LogLine,
}

/// 日志查询后台缓存（UI 与查询线程共享；非 Signal，同步访问）。
#[derive(Default)]
pub struct LogBackend {
    /// 缓存键：日志源的 path 或 command
    pub src_key: String,
    pub index: Option<crate::core::log_index::LogIndex>,
}

/// 进程编辑对话框的信号组（打开时填充，保存时收集）。
#[derive(Clone)]
pub struct EditState {
    pub show: Signal<bool>,
    /// 空 = 新建
    pub editing_id: Signal<String>,
    pub name: Signal<String>,
    pub ptype: Signal<usize>,
    pub observe_name: Signal<String>,
    pub start_cmd: Signal<String>,
    pub stop_cmd: Signal<String>,
    pub check_cmd: Signal<String>,
    pub install_cmd: Signal<String>,
    pub detached: Signal<bool>,
    pub work_dir: Signal<String>,
    /// K=V 每行一对
    pub env_text: Signal<String>,
    /// 服务地址，每行一条 proto://host:port[/path]
    pub addrs_text: Signal<String>,
    /// 日志源，每行：名称 | 路径 | 命令 | 格式(json_lines/logfmt/bracket/plain)
    pub logs_text: Signal<String>,
}

impl EditState {
    pub fn new() -> Self {
        Self {
            show: signal(false),
            editing_id: signal(String::new()),
            name: signal(String::new()),
            ptype: signal(0),
            observe_name: signal(String::new()),
            start_cmd: signal(String::new()),
            stop_cmd: signal(String::new()),
            check_cmd: signal(String::new()),
            install_cmd: signal(String::new()),
            detached: signal(false),
            work_dir: signal(String::new()),
            env_text: signal(String::new()),
            addrs_text: signal(String::new()),
            logs_text: signal(String::new()),
        }
    }
}

/// UI 全局上下文：持有共享状态、tokio 句柄与全部派生 Signal。
/// 注意：含 `Signal`（!Send），只能在 UI 线程使用；后台任务只携带
/// `app`（SharedState）/`rt`/`tx` 等 Send 字段。
#[derive(Clone)]
pub struct UiCtx {
    pub app: crate::state::SharedState,
    pub rt: tokio::runtime::Handle,
    pub tx: windui::prelude::Sender<UiMsg>,
    pub page: Signal<usize>,
    // 仪表盘
    pub d_total: Signal<String>,
    pub d_running: Signal<String>,
    pub d_services: Signal<String>,
    pub d_api: Signal<String>,
    pub d_cpu: Signal<String>,
    pub d_mem: Signal<String>,
    // 进程
    pub rows: Signal<Vec<ProcRow>>,
    pub edit: EditState,
    /// 待删除确认的进程（id, name）（FR-PROC-12）
    pub del_target: Signal<Option<(String, String)>>,
    pub del_show: Signal<bool>,
    // 服务
    pub svc: Signal<Vec<SvcRow>>,
    // 日志
    pub log_proc_names: Signal<Vec<String>>,
    pub log_proc: Signal<usize>,
    pub log_src_names: Signal<Vec<String>>,
    pub log_src: Signal<usize>,
    pub log_q: Signal<String>,
    pub log_tag: Signal<String>,
    pub log_levels: [Signal<bool>; 5],
    pub log_page: Signal<usize>,
    pub log_info: Signal<String>,
    pub log_lines: Signal<Vec<UiLogLine>>,
    pub log_follow: Signal<bool>,
    pub log_backend: std::sync::Arc<std::sync::Mutex<LogBackend>>,
    // 对外接口页
    pub api_status: Signal<String>,
    pub token_show: Signal<bool>,
    pub skill_dir: Signal<String>,
    // 帮助页快速诊断结果
    pub diag_result: Signal<String>,
}

impl UiCtx {
    pub fn new(
        app: crate::state::SharedState,
        rt: tokio::runtime::Handle,
        tx: windui::prelude::Sender<UiMsg>,
    ) -> Self {
        Self {
            app,
            rt,
            tx,
            page: signal(0usize),
            d_total: signal("0".into()),
            d_running: signal("0".into()),
            d_services: signal("0".into()),
            d_api: signal("初始化中…".into()),
            d_cpu: signal("0%".into()),
            d_mem: signal("0%".into()),
            rows: signal(Vec::new()),
            edit: EditState::new(),
            del_target: signal(None),
            del_show: signal(false),
            svc: signal(Vec::new()),
            log_proc_names: signal(Vec::new()),
            log_proc: signal(0),
            log_src_names: signal(Vec::new()),
            log_src: signal(0),
            log_q: signal(String::new()),
            log_tag: signal(String::new()),
            log_levels: [
                signal(false),
                signal(false),
                signal(false),
                signal(false),
                signal(false),
            ],
            log_page: signal(0),
            log_info: signal("选择进程后点击「查询」".into()),
            log_lines: signal(Vec::new()),
            log_follow: signal(false),
            log_backend: std::sync::Arc::new(std::sync::Mutex::new(LogBackend::default())),
            api_status: signal("启动中…".into()),
            token_show: signal(false),
            skill_dir: signal(String::new()),
            diag_result: signal("尚未检测".into()),
        }
    }

    /// UiMsg::Snapshot：从 AppState 重算派生 Signal（UI 线程内调用）。
    pub fn refresh(&self) {
        let cfg = self.app.config.read().unwrap();
        let snap = self.app.snapshot();
        // 仪表盘统计
        let total = cfg.processes.len();
        let mut running = 0usize;
        let mut rows = Vec::new();
        let mut svc = Vec::new();
        for p in &cfg.processes {
            let st = self.app.mgr.status_of(&p.id);
            let state = st.state_or_default();
            if state == State::Running {
                running += 1;
                if p.ptype == ProcessType::WebService {
                    let pid_text = st
                        .pid
                        .or(st.observe_pid)
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "—".into());
                    for a in &p.addresses {
                        svc.push(SvcRow {
                            url: a.url(),
                            proc_name: p.name.clone(),
                            pid_text: pid_text.clone(),
                            openable: a.openable(),
                        });
                    }
                }
            }
            // 关联系统快照：PID 匹配取资源占用（FR-PROC-08/09）
            let pid_any = st.pid.or(st.observe_pid);
            let res_text = snap
                .as_ref()
                .and_then(|s| s.procs.iter().find(|c| Some(c.pid) == pid_any))
                .map(|c| format!("{:.0}% · {}", c.cpu_percent, human_bytes(c.mem_bytes)))
                .unwrap_or_else(|| "—".into());
            rows.push(ProcRow {
                id: p.id.clone(),
                name: p.name.clone(),
                type_label: p.ptype.label().to_string(),
                state_label: st.state_label(),
                pid_text: st
                    .pid
                    .or(st.observe_pid)
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "—".into()),
                res_text,
                running: state == State::Running,
                observe_only: p.is_observe_only(),
                has_start: p.start_command.is_some(),
                has_stop: p.stop_command.is_some() || st.pid.is_some() || st.observe_pid.is_some(),
                has_check: p.check_command.is_some(),
            });
        }
        self.rows.set(rows);
        self.svc.set(svc);
        self.d_total.set(format!("{total}"));
        self.d_running.set(format!("{running}"));
        self.d_services.set(format!(
            "{}",
            cfg.processes
                .iter()
                .filter(|p| p.ptype == ProcessType::WebService)
                .count()
        ));

        // 资源指标
        if let Some(s) = snap {
            self.d_cpu.set(format!("{:.1}%", s.cpu_percent));
            if s.mem_total_bytes > 0 {
                self.d_mem.set(format!(
                    "{:.1}%（{} / {}）",
                    s.mem_used_bytes as f64 / s.mem_total_bytes as f64 * 100.0,
                    human_bytes(s.mem_used_bytes),
                    human_bytes(s.mem_total_bytes)
                ));
            }
        }

        // 对外接口状态
        let http = self
            .app
            .http_port
            .load(std::sync::atomic::Ordering::Relaxed);
        let ws = self.app.ws_port.load(std::sync::atomic::Ordering::Relaxed);
        if http > 0 {
            self.d_api.set(format!("HTTP {http} · WS {ws} 已监听"));
            self.api_status.set(format!(
                "HTTP  http://127.0.0.1:{http}\nWS    ws://127.0.0.1:{ws}/ws?token=…\nMCP   cucumber --mcp（stdio）或 POST http://127.0.0.1:{http}/mcp"
            ));
        }

        // 日志页进程下拉（保持选择有效）
        let candidates: Vec<&ProcessConfig> = cfg
            .processes
            .iter()
            .filter(|p| !p.log_sources.is_empty())
            .collect();
        let names: Vec<String> = candidates.iter().map(|p| p.name.clone()).collect();
        let cur = self.log_proc.get();
        if cur >= names.len() {
            self.log_proc.set(0);
            self.log_src.set(0);
        }
        let sel_proc = cur.min(names.len().saturating_sub(1));
        let src_names: Vec<String> = candidates
            .get(sel_proc)
            .map(|p| p.log_sources.iter().map(|s| s.name.clone()).collect())
            .unwrap_or_default();
        let cur_src = self.log_src.get();
        if cur_src >= src_names.len() {
            self.log_src.set(0);
        }
        self.log_proc_names.set(names);
        self.log_src_names.set(src_names);
    }

    /// 当前日志页选中的进程 id 与日志源配置。
    pub fn log_selection(&self) -> Option<(String, crate::core::config::LogSource)> {
        let cfg = self.app.config.read().unwrap();
        let candidates: Vec<&ProcessConfig> = cfg
            .processes
            .iter()
            .filter(|p| !p.log_sources.is_empty())
            .collect();
        let p = candidates.get(self.log_proc.get().min(candidates.len().saturating_sub(1)))?;
        let s = p.log_sources.get(
            self.log_src
                .get()
                .min(p.log_sources.len().saturating_sub(1)),
        )?;
        Some((p.id.clone(), s.clone()))
    }
}

/// 在 UI 回调里发起后台操作：完成/失败以 toast 回报。
/// 注意：闭包只能捕获 `Send` 数据（Signal 是 !Send，勿把 UiCtx 整个搬进来）。
pub fn spawn_op<F, Fut>(
    tx: windui::prelude::Sender<UiMsg>,
    rt: &tokio::runtime::Handle,
    ok_msg: String,
    f: F,
) where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    rt.spawn(async move {
        match f().await {
            Ok(()) => {
                let _ = tx.send(UiMsg::ToastOk(ok_msg));
            }
            Err(e) => {
                let _ = tx.send(UiMsg::ToastErr(e));
            }
        }
    });
}
