//! 配置进程的状态机与命令执行（FR-PROC-04~13，ADR-03/04）。
//!
//! 与 UI 解耦：UI 与 API 通过 `ProcessManager`（内部 Arc 共享）发起操作、读取状态；
//! 全部命令执行都在 tokio 后台任务中完成。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::core::config::ProcessConfig;
use crate::core::sys_observer::Snapshot;
use crate::platform::commands::{self, CommandOutput, OutputRing};
use crate::platform::signals;

/// 状态机主状态（FR-PROC-13）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

impl State {
    pub fn label(&self) -> &'static str {
        match self {
            State::Stopped => "已停止",
            State::Starting => "启动中",
            State::Running => "运行中",
            State::Stopping => "停止中",
            State::Error => "异常",
        }
    }
}

/// 单个配置进程的运行时状态。
#[derive(Debug, Clone, Default)]
pub struct ProcStatus {
    pub state: Option<State>,
    pub pid: Option<u32>,
    /// 系统进程模式下观察到的 PID（FR-PROC-09：与 pid 区分展示）
    pub observe_pid: Option<u32>,
    pub last_exit: Option<i32>,
    pub last_error: String,
    /// 最近一次命令输出（FR-PROC-06）
    pub last_output: String,
    /// 最近一次 check_command 的健康结论
    pub health: Option<bool>,
    /// 端口探测结论（0.3.1，ADR-14）：配置了地址的服务，任一 TCP/http/https 端口连通为 true。
    /// None = 未配置可探测地址。与状态机融合见 [`ProcStatus::effective_state`]。
    pub port_up: Option<bool>,
}

impl ProcStatus {
    pub fn state_or_default(&self) -> State {
        self.state.unwrap_or(State::Stopped)
    }

    /// 有效状态（0.3.1）：端口探测 up 时无论状态机如何，服务都视为运行中——
    /// 覆盖外部启动（systemctl/手动/工具重启后仍在跑的点火即走服务）的场景。
    pub fn effective_state(&self) -> State {
        if self.port_up == Some(true) {
            State::Running
        } else {
            self.state_or_default()
        }
    }

    /// 是否为「外部启动」：端口通但状态机不认为在跑。
    pub fn externally_running(&self) -> bool {
        self.port_up == Some(true) && self.state_or_default() != State::Running
    }

    pub fn state_label(&self) -> String {
        if self.externally_running() {
            return "运行中（外部）".to_string();
        }
        match self.state_or_default() {
            State::Error if !self.last_error.is_empty() => format!("异常：{}", self.last_error),
            s => s.label().to_string(),
        }
    }
}

struct Inner {
    statuses: RwLock<HashMap<String, ProcStatus>>,
    /// 由本工具 spawn、当前存活的子进程（id → pid）
    running: Mutex<HashMap<String, u32>>,
    /// 长驻子进程的输出环形缓冲（id → ring）
    rings: Mutex<HashMap<String, Arc<Mutex<OutputRing>>>>,
}

/// 进程管理器；`Clone` 即共享同一份状态。
#[derive(Clone)]
pub struct ProcessManager {
    inner: Arc<Inner>,
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                statuses: RwLock::new(HashMap::new()),
                running: Mutex::new(HashMap::new()),
                rings: Mutex::new(HashMap::new()),
            }),
        }
    }

    fn set_state(&self, id: &str, f: impl FnOnce(&mut ProcStatus)) {
        if let Ok(mut map) = self.inner.statuses.write() {
            let st = map.entry(id.to_string()).or_default();
            f(st);
        }
    }

    pub fn status_of(&self, id: &str) -> ProcStatus {
        self.inner
            .statuses
            .read()
            .ok()
            .and_then(|m| m.get(id).cloned())
            .unwrap_or_default()
    }

    pub fn running_output(&self, id: &str, max_lines: usize) -> Vec<String> {
        self.inner
            .rings
            .lock()
            .ok()
            .and_then(|r| r.get(id).cloned())
            .and_then(|ring| ring.lock().ok().map(|r| r.snapshot()))
            .map(|mut v| {
                if v.len() > max_lines {
                    v.drain(v.len() - max_lines..).collect()
                } else {
                    v
                }
            })
            .unwrap_or_default()
    }

    fn busy(&self, id: &str) -> Option<String> {
        match self.status_of(id).state_or_default() {
            State::Starting => Some("正在启动".into()),
            State::Stopping => Some("正在停止".into()),
            _ => None,
        }
    }

    /// 启动（ADR-03：spawn 长驻子进程 / start_detached 点火即走）。
    pub async fn start(&self, cfg: &ProcessConfig) -> Result<(), String> {
        let Some(cmd) = cfg.start_command.as_deref().filter(|s| !s.is_empty()) else {
            if cfg.is_observe_only() {
                return Err("系统进程模式不执行启动命令".into());
            }
            return Err("未配置 start_command".into());
        };
        if let Some(why) = self.busy(&cfg.id) {
            return Err(why);
        }

        self.set_state(&cfg.id, |st| {
            st.state = Some(State::Starting);
            st.last_error.clear();
        });

        let id = cfg.id.clone();
        let cmd = cmd.to_string();
        let cfg = cfg.clone();
        let ring = {
            let mut rings = self.inner.rings.lock().map_err(|e| e.to_string())?;
            rings
                .entry(id.clone())
                .or_insert_with(|| Arc::new(Mutex::new(OutputRing::new(64 * 1024))))
                .clone()
        };
        let this = self.clone();
        let id2 = id.clone();

        if cfg.start_detached {
            tokio::spawn(async move {
                let out = commands::run_wait(&cmd, cfg.work_dir.as_deref(), &cfg.env, 600).await;
                if out.ok() {
                    this.set_state(&id2, |st| {
                        st.state = Some(State::Running);
                        st.last_output = out.summary();
                    });
                } else {
                    this.set_state(&id2, |st| {
                        st.state = Some(State::Error);
                        st.last_error = if out.stderr.is_empty() {
                            format!("退出码 {:?}", out.exit_code)
                        } else {
                            out.stderr.chars().take(200).collect()
                        };
                        st.last_output = out.summary();
                    });
                }
            });
        } else {
            tokio::spawn(async move {
                let this2 = this.clone();
                let id3 = id2.clone();
                let spawned = commands::spawn_child(
                    &cmd,
                    cfg.work_dir.as_deref(),
                    &cfg.env,
                    ring,
                    Box::new(move |code| {
                        // 子进程退出：Stopping → Stopped；Running → 意外退出
                        this2
                            .inner
                            .running
                            .lock()
                            .ok()
                            .and_then(|mut r| r.remove(&id3));
                        this2.set_state(&id3, |st| {
                            st.pid = None;
                            st.last_exit = code;
                            st.state = Some(match st.state_or_default() {
                                State::Stopping => State::Stopped,
                                s if s == State::Running || s == State::Starting => {
                                    if code == Some(0) {
                                        State::Stopped
                                    } else {
                                        st.last_error = format!("子进程异常退出（码 {code:?}）");
                                        State::Error
                                    }
                                }
                                other => other,
                            });
                        });
                    }),
                )
                .await;
                match spawned {
                    Ok(pid) => {
                        if let Ok(mut r) = this.inner.running.lock() {
                            r.insert(id2.clone(), pid);
                        }
                        this.set_state(&id2, |st| {
                            st.pid = Some(pid);
                            st.state = Some(State::Running);
                        });
                    }
                    Err(e) => {
                        this.set_state(&id2, |st| {
                            st.state = Some(State::Error);
                            st.last_error = e;
                        });
                    }
                }
            });
        }
        Ok(())
    }

    /// 停止（ADR-04：stop_command 优先 → SIGTERM+宽限 → 强杀）。
    pub async fn stop(&self, cfg: &ProcessConfig) -> Result<(), String> {
        if let Some(why) = self.busy(&cfg.id) {
            return Err(why);
        }
        let state = self.status_of(&cfg.id).state_or_default();
        if state == State::Stopped {
            return Ok(());
        }
        self.set_state(&cfg.id, |st| st.state = Some(State::Stopping));

        // 1) stop_command（wait 语义）
        if let Some(cmd) = cfg.stop_command.as_deref().filter(|s| !s.is_empty()) {
            let out = commands::run_wait(cmd, cfg.work_dir.as_deref(), &cfg.env, 600).await;
            self.set_state(&cfg.id, |st| st.last_output = out.summary());
            if out.ok() {
                self.set_state(&cfg.id, |st| {
                    st.state = Some(State::Stopped);
                    st.pid = None;
                });
                if let Ok(mut r) = self.inner.running.lock() {
                    r.remove(&cfg.id);
                }
                return Ok(());
            }
            return Err(format!("stop_command 失败：{}", out.summary()));
        }

        // 2) 本工具 spawn 的子进程：SIGTERM → 5s 宽限 → SIGKILL
        let pid = self
            .inner
            .running
            .lock()
            .ok()
            .and_then(|r| r.get(&cfg.id).copied());
        if let Some(pid) = pid {
            signals::terminate_pid(pid).await;
            if self.wait_gone(&cfg.id, 50).await {
                return Ok(());
            }
            signals::kill_pid(pid).await;
            if self.wait_gone(&cfg.id, 20).await {
                return Ok(());
            }
            return Err("强制终止后进程仍存活".into());
        }

        // 3) 观察模式：尽力而为信号（R-05）
        if let Some(pid) = self.status_of(&cfg.id).observe_pid {
            let ok = signals::terminate_pid(pid).await;
            self.set_state(&cfg.id, |st| st.state = Some(State::Stopped));
            return if ok {
                Ok(())
            } else {
                Err("信号发送失败（尽力而为）".into())
            };
        }

        self.set_state(&cfg.id, |st| st.state = Some(State::Stopped));
        Ok(())
    }

    /// 等待 running 表中该 id 消失（每 100ms 一次，共 tries 次）。
    async fn wait_gone(&self, id: &str, tries: usize) -> bool {
        for _ in 0..tries {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let gone = self
                .inner
                .running
                .lock()
                .map(|r| !r.contains_key(id))
                .unwrap_or(true);
            if gone {
                self.set_state(id, |st| {
                    st.state = Some(State::Stopped);
                    st.pid = None;
                });
                return true;
            }
        }
        false
    }

    /// 健康检查：优先 check_command（退出码 0 = 健康）；
    /// 未配置命令但配置了服务地址时，退化为端口探测（0.3.1，ADR-14）。
    pub async fn check(&self, cfg: &ProcessConfig) -> Result<CommandOutput, String> {
        if let Some(cmd) = cfg.check_command.as_deref().filter(|s| !s.is_empty()) {
            let out = commands::run_wait(cmd, cfg.work_dir.as_deref(), &cfg.env, 120).await;
            self.set_state(&cfg.id, |st| {
                st.health = Some(out.ok());
                st.last_output = out.summary();
            });
            return Ok(out);
        }
        // 端口探测兜底
        let up = crate::core::port_probe::proc_port_up(&cfg.addresses)
            .await
            .ok_or_else(|| "未配置 check_command，也未配置可探测的服务地址".to_string())?;
        self.set_state(&cfg.id, |st| {
            st.health = Some(up);
            st.port_up = Some(up);
            st.last_output = format!("端口探测：{}", if up { "连通" } else { "不通" });
        });
        Ok(CommandOutput {
            exit_code: Some(if up { 0 } else { 1 }),
            stdout: format!("端口探测：{}", if up { "连通" } else { "不通" }),
            stderr: String::new(),
        })
    }

    /// 安装依赖。
    pub async fn install_deps(&self, cfg: &ProcessConfig) -> Result<CommandOutput, String> {
        let Some(cmd) = cfg
            .install_deps_command
            .as_deref()
            .filter(|s| !s.is_empty())
        else {
            return Err("未配置 install_deps_command".into());
        };
        let out = commands::run_wait(cmd, cfg.work_dir.as_deref(), &cfg.env, 3600).await;
        self.set_state(&cfg.id, |st| st.last_output = out.summary());
        Ok(out)
    }

    /// 写入端口探测结论（0.3.1，ADR-14；由探测任务周期调用）。
    pub fn set_port_up(&self, id: &str, up: bool) {
        self.set_state(id, |st| st.port_up = Some(up));
    }

    /// 用系统快照刷新观察模式进程的状态（FR-PROC-07/13）。
    pub fn refresh_observe(&self, cfg: &ProcessConfig, snap: &Snapshot) {
        let Some(name) = cfg.observe_name.as_deref().filter(|s| !s.is_empty()) else {
            return;
        };
        let found = snap
            .procs
            .iter()
            .find(|p| p.name.to_lowercase().contains(&name.to_lowercase()));
        self.set_state(&cfg.id, |st| match found {
            Some(p) => {
                st.observe_pid = Some(p.pid);
                st.state = Some(State::Running);
            }
            None => {
                st.observe_pid = None;
                st.state = Some(State::Stopped);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::ProcessType;

    fn sleep_cfg(detached: bool) -> ProcessConfig {
        ProcessConfig {
            id: "t1".into(),
            name: "测试睡眠".into(),
            ptype: ProcessType::Generic,
            start_command: Some(
                #[cfg(unix)]
                "sleep 30".into(),
                #[cfg(windows)]
                "ping -n 30 127.0.0.1".into(),
            ),
            stop_command: None,
            check_command: Some(
                #[cfg(unix)]
                "true".into(),
                #[cfg(windows)]
                "cmd /C exit 0".into(),
            ),
            install_deps_command: None,
            start_detached: detached,
            work_dir: None,
            env: Default::default(),
            observe_name: None,
            addresses: vec![],
            log_sources: vec![],
        }
    }

    async fn wait_state(mgr: &ProcessManager, id: &str, want: State, tries: usize) -> bool {
        for _ in 0..tries {
            if mgr.status_of(id).state_or_default() == want {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        false
    }

    #[tokio::test]
    async fn spawn_stop_lifecycle() {
        let mgr = ProcessManager::new();
        let cfg = sleep_cfg(false);
        mgr.start(&cfg).await.unwrap();
        assert!(
            wait_state(&mgr, "t1", State::Running, 100).await,
            "应进入运行中"
        );
        assert!(mgr.status_of("t1").pid.is_some());
        mgr.stop(&cfg).await.unwrap();
        assert!(wait_state(&mgr, "t1", State::Stopped, 100).await, "应停止");
    }

    #[tokio::test]
    async fn detached_exit_zero_is_running() {
        let mgr = ProcessManager::new();
        let mut cfg = sleep_cfg(true);
        cfg.start_command = Some(
            #[cfg(unix)]
            "true".into(),
            #[cfg(windows)]
            "cmd /C exit 0".into(),
        );
        mgr.start(&cfg).await.unwrap();
        assert!(
            wait_state(&mgr, "t1", State::Running, 100).await,
            "点火即走：码 0 → 运行中"
        );
    }

    #[tokio::test]
    async fn bad_command_is_error() {
        let mgr = ProcessManager::new();
        let mut cfg = sleep_cfg(false);
        cfg.start_command = Some("definitely-not-exist-xyz".into());
        mgr.start(&cfg).await.unwrap();
        assert!(
            wait_state(&mgr, "t1", State::Error, 100).await,
            "失败命令 → 异常"
        );
        assert!(!mgr.status_of("t1").last_error.is_empty());
    }

    #[tokio::test]
    async fn check_health() {
        let mgr = ProcessManager::new();
        let cfg = sleep_cfg(false);
        let out = mgr.check(&cfg).await.unwrap();
        assert!(out.ok());
        assert_eq!(mgr.status_of("t1").health, Some(true));
    }

    #[test]
    fn effective_state_fuses_port_probe() {
        let mut st = ProcStatus {
            state: Some(State::Stopped),
            ..Default::default()
        };
        // 未配置地址：port_up=None → 沿用状态机
        assert_eq!(st.effective_state(), State::Stopped);
        assert!(!st.externally_running());
        // 端口通 + 状态机 Stopped → 运行中（外部）
        st.port_up = Some(true);
        assert_eq!(st.effective_state(), State::Running);
        assert!(st.externally_running());
        assert_eq!(st.state_label(), "运行中（外部）");
        // 状态机 Running + 端口通 → 正常运行（非外部）
        st.state = Some(State::Running);
        assert!(!st.externally_running());
        // 设计边界：端口 down 不推翻状态机（进程可能活着但端口未就绪）——
        // 端口探测只用于「发现运行」，不用于「否定运行」
        st.state = Some(State::Stopped);
        st.port_up = Some(false);
        assert_eq!(st.effective_state(), State::Stopped);
    }

    #[test]
    fn observe_refresh() {
        let mgr = ProcessManager::new();
        let mut cfg = sleep_cfg(false);
        cfg.start_command = None;
        cfg.observe_name = Some("cucumber".into());
        let mut obs = crate::core::sys_observer::SysObserver::new();
        let snap = obs.refresh();
        mgr.refresh_observe(&cfg, &snap);
        // 测试进程自身必然可观察到（跨平台稳定；无 systemd 的容器/CI 亦可）
        assert_eq!(mgr.status_of("t1").state_or_default(), State::Running);
        // 不存在的名字 → Stopped
        cfg.observe_name = Some("no-such-proc-xyz".into());
        mgr.refresh_observe(&cfg, &snap);
        assert_eq!(mgr.status_of("t1").state_or_default(), State::Stopped);
    }
}
