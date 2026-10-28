//! 统一操作层（FR-API-02）：HTTP handler 与 MCP tools 调用同一组函数，保证能力对等。

use serde_json::{json, Value};

use crate::core::config::ProcessConfig;
use crate::core::log_index::{LogFilter, LogIndex};
use crate::core::log_parser::Level;
use crate::core::skill_installer;
use crate::state::SharedState;

pub struct Ops {
    pub state: SharedState,
}

fn proc_view(cfg: &ProcessConfig, state: &SharedState) -> Value {
    let st = state.mgr.status_of(&cfg.id);
    json!({
        "id": cfg.id,
        "name": cfg.name,
        "type": cfg.ptype,
        "type_label": cfg.ptype.label(),
        "state": st.state_or_default().label(),
        "state_raw": format!("{:?}", st.state_or_default()),
        "pid": st.pid,
        "observe_pid": st.observe_pid,
        "observe_only": cfg.is_observe_only(),
        "start_command": cfg.start_command,
        "stop_command": cfg.stop_command,
        "check_command": cfg.check_command,
        "install_deps_command": cfg.install_deps_command,
        "start_detached": cfg.start_detached,
        "work_dir": cfg.work_dir,
        "env": cfg.env,
        "addresses": cfg.addresses.iter().map(|a| json!({
            "protocol": a.protocol, "host": a.host, "port": a.port,
            "path": a.path, "url": a.url(),
        })).collect::<Vec<_>>(),
        "log_sources": cfg.log_sources.iter().map(|s| &s.name).collect::<Vec<_>>(),
        "last_exit": st.last_exit,
        "last_error": st.last_error,
        "health": st.health,
    })
}

impl Ops {
    pub fn list_processes(&self) -> Vec<Value> {
        let cfg = self.state.config.read().unwrap();
        cfg.processes
            .iter()
            .map(|p| proc_view(p, &self.state))
            .collect()
    }

    pub fn get_process(&self, id: &str) -> Option<Value> {
        let cfg = self.state.config.read().unwrap();
        cfg.find(id).map(|p| {
            let mut v = proc_view(p, &self.state);
            v["recent_output"] = json!(self.state.mgr.running_output(id, 50));
            v
        })
    }

    pub async fn start(&self, id: &str) -> Result<Value, String> {
        let cfg = {
            let c = self.state.config.read().unwrap();
            c.find(id)
                .cloned()
                .ok_or_else(|| format!("进程不存在：{id}"))?
        };
        self.state.mgr.start(&cfg).await?;
        Ok(json!({"id": id, "action": "start", "accepted": true}))
    }

    pub async fn stop(&self, id: &str) -> Result<Value, String> {
        let cfg = {
            let c = self.state.config.read().unwrap();
            c.find(id)
                .cloned()
                .ok_or_else(|| format!("进程不存在：{id}"))?
        };
        self.state.mgr.stop(&cfg).await?;
        Ok(json!({"id": id, "action": "stop", "done": true}))
    }

    pub async fn check(&self, id: &str) -> Result<Value, String> {
        let cfg = {
            let c = self.state.config.read().unwrap();
            c.find(id)
                .cloned()
                .ok_or_else(|| format!("进程不存在：{id}"))?
        };
        let out = self.state.mgr.check(&cfg).await?;
        Ok(json!({
            "id": id, "healthy": out.ok(), "exit_code": out.exit_code,
            "output": out.summary()
        }))
    }

    pub async fn install_deps(&self, id: &str) -> Result<Value, String> {
        let cfg = {
            let c = self.state.config.read().unwrap();
            c.find(id)
                .cloned()
                .ok_or_else(|| format!("进程不存在：{id}"))?
        };
        let out = self.state.mgr.install_deps(&cfg).await?;
        Ok(json!({
            "id": id, "ok": out.ok(), "exit_code": out.exit_code,
            "output": out.summary()
        }))
    }

    /// upsert（POST 无 id 新建 / PUT 按 id 更新）。
    pub fn upsert_process(&self, cfg: ProcessConfig) -> Result<Value, String> {
        if cfg.name.trim().is_empty() {
            return Err("进程名称不能为空".into());
        }
        let id = {
            let mut c = self.state.config.write().unwrap();
            let id = if cfg.id.is_empty() {
                crate::core::config::next_process_id(&c.processes)
            } else {
                cfg.id.clone()
            };
            let mut with_id = cfg;
            with_id.id = id.clone();
            c.upsert_process(with_id);
            id
        };
        self.state.save_config()?;
        Ok(json!({"id": id, "saved": true}))
    }

    pub fn delete_process(&self, id: &str) -> Result<Value, String> {
        {
            let mut c = self.state.config.write().unwrap();
            if !c.remove_process(id) {
                return Err(format!("进程不存在：{id}"));
            }
        }
        self.state.save_config()?;
        Ok(json!({"id": id, "deleted": true}))
    }

    /// 所有服务（FR-SERV-01/02：v1 = 运行中的 web_service 地址展开）。
    pub fn services(&self) -> Vec<Value> {
        let cfg = self.state.config.read().unwrap();
        cfg.processes
            .iter()
            .filter(|p| p.ptype == crate::core::config::ProcessType::WebService)
            .filter(|p| {
                let st = self.state.mgr.status_of(&p.id);
                st.state_or_default() == crate::core::process_manager::State::Running
            })
            .flat_map(|p| {
                let st = self.state.mgr.status_of(&p.id);
                let pid = st.pid.or(st.observe_pid);
                p.addresses.iter().map(move |a| {
                    json!({
                        "process_id": p.id,
                        "process_name": p.name,
                        "pid": pid,
                        "protocol": a.protocol, "host": a.host, "port": a.port,
                        "path": a.path, "url": a.url(),
                    })
                })
            })
            .collect()
    }

    /// 仪表盘（FR-DASH-01）。
    pub fn dashboard(&self) -> Value {
        let cfg = self.state.config.read().unwrap();
        let total = cfg.processes.len();
        let running = cfg
            .processes
            .iter()
            .filter(|p| {
                self.state.mgr.status_of(&p.id).state_or_default()
                    == crate::core::process_manager::State::Running
            })
            .count();
        let services = cfg
            .processes
            .iter()
            .filter(|p| p.ptype == crate::core::config::ProcessType::WebService)
            .count();
        let snap = self.state.snapshot();
        let (cpu, mem_used, mem_total) = snap
            .as_ref()
            .map(|s| (s.cpu_percent, s.mem_used_bytes, s.mem_total_bytes))
            .unwrap_or((0.0, 0, 0));
        let http = self
            .state
            .http_port
            .load(std::sync::atomic::Ordering::Relaxed);
        let ws = self
            .state
            .ws_port
            .load(std::sync::atomic::Ordering::Relaxed);
        json!({
            "processes_total": total,
            "processes_running": running,
            "web_services_configured": services,
            "api": {"http_port": http, "ws_port": ws, "listening": http > 0},
            "cpu_percent": (cpu * 10.0).round() / 10.0,
            "mem_used_bytes": mem_used,
            "mem_total_bytes": mem_total,
            "mem_percent": if mem_total > 0 {
                (mem_used as f64 / mem_total as f64 * 1000.0).round() / 10.0
            } else { 0.0 },
        })
    }

    /// 日志查询（FR-LOG-03/05；阻塞索引构建放 spawn_blocking）。
    #[allow(clippy::too_many_arguments)] // 查询参数天然多维，强拆结构体反而绕
    pub async fn query_logs(
        &self,
        id: &str,
        source: Option<String>,
        q: Option<String>,
        levels: Vec<String>,
        tag: Option<String>,
        ts_from: Option<i64>,
        ts_to: Option<i64>,
        page: usize,
        page_size: Option<usize>,
    ) -> Result<Value, String> {
        let src = {
            let c = self.state.config.read().unwrap();
            let p = c.find(id).ok_or_else(|| format!("进程不存在：{id}"))?;
            match source {
                Some(name) => p
                    .log_sources
                    .iter()
                    .find(|s| s.name == name)
                    .cloned()
                    .ok_or_else(|| format!("日志源不存在：{name}"))?,
                None => p.log_sources.first().cloned().ok_or("该进程未配置日志源")?,
            }
        };
        let page_size = page_size.unwrap_or_else(|| {
            self.state
                .config
                .read()
                .map(|c| c.general.log_page_size)
                .unwrap_or(100)
        });
        let filter = LogFilter {
            levels: levels.iter().filter_map(|l| Level::parse(l)).collect(),
            tag,
            ts_from,
            ts_to,
            query: q,
        };
        tokio::task::spawn_blocking(move || {
            let idx = LogIndex::open(&src)?;
            let (hits, tags) = idx.filter(&filter);
            let pg = idx.page(&hits, page, page_size);
            Ok(json!({
                "total_lines": pg.total_lines,
                "total_hits": pg.total_hits,
                "page": pg.page,
                "page_size": pg.page_size,
                "tags": tags,
                "lines": pg.lines.iter().map(|(no, l)| json!({
                    "no": no,
                    "raw": l.raw,
                    "level": l.level.map(|v| v.label()),
                    "tags": l.tags,
                    "ts": l.ts.map(crate::core::log_parser::format_ts),
                    "message": l.message,
                })).collect::<Vec<_>>(),
            }))
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// 安装内置 skill（FR-SKILL）。
    pub fn install_skill(&self, target: Option<String>) -> Result<Value, String> {
        let dir = target.map(std::path::PathBuf::from).or_else(|| {
            self.state
                .config
                .read()
                .ok()
                .and_then(|c| c.general.skill_dir.clone().map(std::path::PathBuf::from))
        });
        let p = skill_installer::install_builtin(dir.as_deref())?;
        Ok(json!({
            "installed": true,
            "path": p.to_string_lossy(),
            "name": skill_installer::BUILTIN_SKILL_NAME
        }))
    }
}
