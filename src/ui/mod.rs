//! UI 层：windui 页面与全局上下文（NFR-MAINT-03：经 `kit` 适配层收口框架调用）。

pub mod about;
pub mod api_page;
pub mod app;
pub mod dashboard;
pub mod docs;
pub mod gauge;
pub mod i18n;
pub mod icons;
pub mod kit;
pub mod logs;
pub mod processes;
pub mod services;
pub mod settings;

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
    /// 发现新版本（版本号、Release 地址）（任务 7）
    UpdateAvailable(String, String),
    /// 托盘事件（任务 5）
    TrayShow,
    TrayQuit,
    /// 系统字体枚举完成（任务 2）
    FontsLoaded(Vec<String>),
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
    // 字体选择（任务 2）
    pub font_dialog: Signal<bool>,
    pub font_list: Signal<Vec<String>>,
    // 语言与文案（任务 3）
    pub lang: Signal<i18n::Lang>,
    /// 语言下拉选中索引（dropdown 无回调，经 1s tick 桥接到 set_lang）
    pub lang_sel: Signal<usize>,
    pub t: Texts,
    // 主题/设置（任务 3）
    pub theme: windui::app::ThemeHandle,
    pub theme_mode: Signal<usize>, // 0 浅色 1 深色 2 跟随系统
    pub font_family: Signal<String>,
    pub font_scale: Signal<f32>,
    /// 字号滑条桥接信号（0..1 ↔ 0.8..1.5，经 tick 应用）
    pub font_norm: Signal<f32>,
    pub font_pct: Signal<String>,
    pub bg_use: Signal<bool>,
    pub bg_color: Signal<windui::geometry::Color>,
    pub tray_enabled: Signal<bool>,
    // 仪表数据（任务 5）
    pub g_cpu: Signal<f32>,
    pub g_mem: Signal<f32>,
    pub g_disk: Signal<f32>,
    pub g_net: Signal<f32>,
    pub sub_cpu: Signal<String>,
    pub sub_mem: Signal<String>,
    pub sub_disk: Signal<String>,
    pub sub_net: Signal<String>,
    pub sub_gpu: Signal<String>,
    pub g_gpu: Signal<f32>,
    // 更新提醒（任务 7）
    pub update_show: Signal<bool>,
    pub update_ver: Signal<String>,
    pub update_url: Signal<String>,
    // 托盘统计文案（弹出时现取，任务 8）
    pub tray_stats: Signal<String>,
    pub tray: Option<std::sync::Arc<crate::platform::tray::TrayBridge>>,
}

/// 全部界面文案信号（语言切换时由 apply_lang 统一重写）。
#[derive(Clone)]
pub struct Texts {
    pub nav_dashboard: Signal<String>,
    pub nav_processes: Signal<String>,
    pub nav_services: Signal<String>,
    pub nav_logs: Signal<String>,
    pub nav_api: Signal<String>,
    pub nav_docs: Signal<String>,
    pub nav_help: Signal<String>,
    pub nav_settings: Signal<String>,
    pub c_add: Signal<String>,
    pub c_copy: Signal<String>,
    pub c_open: Signal<String>,
    pub c_query: Signal<String>,
    pub sv_copy: Signal<String>,
    pub sv_hint: Signal<String>,
    pub sv_empty: Signal<String>,
    pub l_search: Signal<String>,
    pub l_tag: Signal<String>,
    pub l_level: Signal<String>,
    pub l_follow: Signal<String>,
    pub l_empty: Signal<String>,
    pub d_cpu: Signal<String>,
    pub d_mem: Signal<String>,
    pub d_disk: Signal<String>,
    pub d_net: Signal<String>,
    pub d_gpu: Signal<String>,
    pub d_procs: Signal<String>,
    pub d_running: Signal<String>,
    pub d_svcs: Signal<String>,
    pub d_api: Signal<String>,
    pub d_hint: Signal<String>,
    pub p_empty: Signal<String>,
    pub p_auto: Signal<String>,
    pub p_delwarn: Signal<String>,
    pub p_deltitle: Signal<String>,
    pub u_title: Signal<String>,
    pub u_now: Signal<String>,
    pub u_later: Signal<String>,
    pub u_skip: Signal<String>,
    pub c_cancel: Signal<String>,
    pub c_confirm: Signal<String>,
    pub m_file: Signal<String>,
    pub m_view: Signal<String>,
    pub m_help: Signal<String>,
    pub m_exit: Signal<String>,
    pub u_browse: Signal<String>,
    pub u_install: Signal<String>,
    pub u_installing: Signal<String>,
    pub f_name: Signal<String>,
    pub f_type: Signal<String>,
    pub f_observe: Signal<String>,
    pub f_cmdgrp: Signal<String>,
    pub f_startcmd: Signal<String>,
    pub f_stopcmd: Signal<String>,
    pub f_checkcmd: Signal<String>,
    pub f_instcmd: Signal<String>,
    pub f_detached: Signal<String>,
    pub f_workdir: Signal<String>,
    pub f_env: Signal<String>,
    pub f_addrs: Signal<String>,
    pub f_logs: Signal<String>,
    pub f_editortitle: Signal<String>,
}

impl Texts {
    fn new(lang: i18n::Lang) -> Self {
        let f = |k: &'static str| signal(i18n::tr(lang, k));
        Self {
            nav_dashboard: f("nav.dashboard"),
            nav_processes: f("nav.processes"),
            nav_services: f("nav.services"),
            nav_logs: f("nav.logs"),
            nav_api: f("nav.api"),
            nav_docs: f("nav.docs"),
            nav_help: f("nav.help"),
            nav_settings: f("nav.settings"),
            c_add: f("c.add"),
            c_copy: f("c.copy"),
            c_open: f("c.open"),
            c_query: f("c.query"),
            sv_copy: f("sv.copy"),
            sv_hint: f("sv.hint"),
            sv_empty: f("sv.empty"),
            l_search: f("l.search"),
            l_tag: f("l.tag"),
            l_level: f("l.level"),
            l_follow: f("l.follow"),
            l_empty: f("l.empty"),
            d_cpu: f("d.cpu"),
            d_mem: f("d.mem"),
            d_disk: f("d.disk"),
            d_net: f("d.net"),
            d_gpu: f("d.gpu"),
            d_procs: f("d.procs"),
            d_running: f("d.running"),
            d_svcs: f("d.svcs"),
            d_api: f("d.api"),
            d_hint: f("d.hint"),
            p_empty: f("p.empty"),
            p_auto: f("p.auto"),
            p_delwarn: f("p.delwarn"),
            p_deltitle: f("p.deltitle"),
            u_title: f("u.title"),
            u_now: f("u.now"),
            u_later: f("u.later"),
            u_skip: f("u.skip"),
            c_cancel: f("c.cancel"),
            c_confirm: f("c.confirm"),
            m_file: f("m.file"),
            m_view: f("m.view"),
            m_help: f("m.help"),
            m_exit: f("m.exit"),
            u_browse: f("u.browse"),
            u_install: f("u.install"),
            u_installing: f("u.installing"),
            f_name: f("f.name"),
            f_type: f("f.type"),
            f_observe: f("f.observe"),
            f_cmdgrp: f("f.cmdgrp"),
            f_startcmd: f("f.startcmd"),
            f_stopcmd: f("f.stopcmd"),
            f_checkcmd: f("f.checkcmd"),
            f_instcmd: f("f.instcmd"),
            f_detached: f("f.detached"),
            f_workdir: f("f.workdir"),
            f_env: f("f.env"),
            f_addrs: f("f.addrs"),
            f_logs: f("f.logs"),
            f_editortitle: f("f.editortitle"),
        }
    }

    fn apply(&self, lang: i18n::Lang) {
        let pairs: Vec<(&Signal<String>, &'static str)> = vec![
            (&self.nav_dashboard, "nav.dashboard"),
            (&self.nav_processes, "nav.processes"),
            (&self.nav_services, "nav.services"),
            (&self.nav_logs, "nav.logs"),
            (&self.nav_api, "nav.api"),
            (&self.nav_docs, "nav.docs"),
            (&self.nav_help, "nav.help"),
            (&self.nav_settings, "nav.settings"),
            (&self.c_add, "c.add"),
            (&self.c_copy, "c.copy"),
            (&self.c_open, "c.open"),
            (&self.c_query, "c.query"),
            (&self.sv_copy, "sv.copy"),
            (&self.sv_hint, "sv.hint"),
            (&self.sv_empty, "sv.empty"),
            (&self.l_search, "l.search"),
            (&self.l_tag, "l.tag"),
            (&self.l_level, "l.level"),
            (&self.l_follow, "l.follow"),
            (&self.l_empty, "l.empty"),
            (&self.d_cpu, "d.cpu"),
            (&self.d_mem, "d.mem"),
            (&self.d_disk, "d.disk"),
            (&self.d_net, "d.net"),
            (&self.d_gpu, "d.gpu"),
            (&self.d_procs, "d.procs"),
            (&self.d_running, "d.running"),
            (&self.d_svcs, "d.svcs"),
            (&self.d_api, "d.api"),
            (&self.d_hint, "d.hint"),
            (&self.p_empty, "p.empty"),
            (&self.p_auto, "p.auto"),
            (&self.p_delwarn, "p.delwarn"),
            (&self.p_deltitle, "p.deltitle"),
            (&self.u_title, "u.title"),
            (&self.u_now, "u.now"),
            (&self.u_later, "u.later"),
            (&self.u_skip, "u.skip"),
            (&self.c_cancel, "c.cancel"),
            (&self.c_confirm, "c.confirm"),
            (&self.m_file, "m.file"),
            (&self.m_view, "m.view"),
            (&self.m_help, "m.help"),
            (&self.m_exit, "m.exit"),
            (&self.u_browse, "u.browse"),
            (&self.u_install, "u.install"),
            (&self.u_installing, "u.installing"),
            (&self.f_name, "f.name"),
            (&self.f_type, "f.type"),
            (&self.f_observe, "f.observe"),
            (&self.f_cmdgrp, "f.cmdgrp"),
            (&self.f_startcmd, "f.startcmd"),
            (&self.f_stopcmd, "f.stopcmd"),
            (&self.f_checkcmd, "f.checkcmd"),
            (&self.f_instcmd, "f.instcmd"),
            (&self.f_detached, "f.detached"),
            (&self.f_workdir, "f.workdir"),
            (&self.f_env, "f.env"),
            (&self.f_addrs, "f.addrs"),
            (&self.f_logs, "f.logs"),
            (&self.f_editortitle, "f.editortitle"),
        ];
        for (sig, key) in pairs {
            sig.set(i18n::tr(lang, key));
        }
    }
}

impl UiCtx {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app: crate::state::SharedState,
        rt: tokio::runtime::Handle,
        tx: windui::prelude::Sender<UiMsg>,
        theme: windui::app::ThemeHandle,
    ) -> Self {
        let cfg = app.config.read().unwrap();
        let lang = i18n::Lang::from_code(&cfg.general.lang);
        let theme_mode = match cfg.general.theme_mode.as_str() {
            "light" => 0,
            "dark" => 1,
            _ => 2,
        };
        let font_family = cfg.general.font_family.clone().unwrap_or_default();
        let font_scale = cfg.general.font_scale.clamp(0.8, 1.5);
        let bg = cfg
            .general
            .bg_color
            .as_deref()
            .and_then(parse_hex_rgba)
            .unwrap_or(windui::geometry::Color::rgb(0xF5, 0xF6, 0xFA));
        let bg_use = cfg.general.bg_color.is_some();
        let tray_enabled = cfg.general.tray_enabled;
        drop(cfg);
        Self {
            lang: signal(lang),
            lang_sel: signal(i18n::Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0)),
            t: Texts::new(lang),
            theme,
            theme_mode: signal(theme_mode),
            font_family: signal(font_family),
            font_scale: signal(font_scale),
            font_norm: signal((font_scale - 0.8) / 0.7),
            font_pct: signal(format!("{}%", (font_scale * 100.0).round() as i32)),
            bg_use: signal(bg_use),
            bg_color: signal(bg),
            tray_enabled: signal(tray_enabled),
            g_cpu: signal(0.0),
            g_mem: signal(0.0),
            g_disk: signal(0.0),
            g_net: signal(0.0),
            sub_cpu: signal("—".into()),
            sub_mem: signal("—".into()),
            sub_disk: signal("—".into()),
            sub_net: signal("—".into()),
            sub_gpu: signal("N/A".into()),
            g_gpu: signal(0.0),
            update_show: signal(false),
            update_ver: signal(String::new()),
            update_url: signal(String::new()),
            tray_stats: signal(String::new()),
            tray: None,
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
            font_dialog: signal(false),
            font_list: signal(Vec::new()),
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
            // 0.3.1：端口探测连通的外部服务也进入服务列表
            let state = st.effective_state();
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

        // 仪表数据（任务 5）
        if let Some(sn) = self.app.snapshot() {
            self.g_cpu.set(sn.cpu_percent / 100.0);
            if sn.mem_total_bytes > 0 {
                self.g_mem
                    .set(sn.mem_used_bytes as f32 / sn.mem_total_bytes as f32);
            }
            self.sub_mem.set(format!(
                "{} / {}",
                human_bytes(sn.mem_used_bytes),
                human_bytes(sn.mem_total_bytes)
            ));
            self.sub_cpu.set(format!("{:.1}%", sn.cpu_percent));
            if sn.disk_total_bytes > 0 {
                self.g_disk
                    .set(sn.disk_used_bytes as f32 / sn.disk_total_bytes as f32);
                self.sub_disk.set(format!(
                    "{} / {}",
                    human_bytes(sn.disk_used_bytes),
                    human_bytes(sn.disk_total_bytes)
                ));
            } else {
                self.sub_disk.set("—".into());
            }
            match sn.gpu_percent {
                Some(g) => {
                    self.g_gpu.set(g / 100.0);
                    self.sub_gpu.set(format!("{g:.0}%"));
                }
                None => {
                    self.g_gpu.set(0.0);
                    self.sub_gpu.set("N/A".into());
                }
            }
            self.sub_net.set(format!(
                "↓{}/s ↑{}/s",
                human_bytes(sn.net_down_bps),
                human_bytes(sn.net_up_bps)
            ));
        }
        // 托盘统计文案（弹出时现取，任务 8）
        {
            let running = self.d_running.get();
            let cpu = self.sub_cpu.get();
            let mem = self.sub_mem.get();
            let svcs = self.d_services.get();
            self.tray_stats
                .set(format!("{running} · {svcs} · CPU {cpu} · MEM {mem}"));
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

/// `#RRGGBBAA` → Color。
pub fn parse_hex_rgba(s: &str) -> Option<windui::geometry::Color> {
    let h = s.trim_start_matches('#');
    if h.len() != 8 && h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    if h.len() == 6 {
        return Some(windui::geometry::Color::rgb(
            (v >> 16) as u8,
            (v >> 8) as u8,
            v as u8,
        ));
    }
    Some(windui::geometry::Color::rgba(
        (v >> 24) as u8,
        (v >> 16) as u8,
        (v >> 8) as u8,
        v as u8,
    ))
}

/// Color → `#RRGGBBAA`。
pub fn to_hex_rgba(c: windui::geometry::Color) -> String {
    format!("#{:02X}{:02X}{:02X}{:02X}", c.r, c.g, c.b, c.a)
}

/// 切换语言并刷新全部文案信号（任务 3）。
pub fn set_lang(ui: &UiCtx, lang: i18n::Lang) {
    ui.lang.set(lang);
    ui.t.apply(lang);
    // 持久化
    if let Ok(mut c) = ui.app.config.write() {
        c.general.lang = lang.code().to_string();
    }
    let _ = ui.app.save_config();
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
