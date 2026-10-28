//! 配置模型与持久化（NFR-MAINT-01：TOML + 平台标准目录）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 进程类型（FR-PROC-01）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProcessType {
    /// 普通进程
    #[default]
    Generic,
    /// Web 服务（带地址列表）
    WebService,
    /// 其它（预留，行为同普通进程）
    Other,
}

impl ProcessType {
    pub fn label(&self) -> &'static str {
        match self {
            ProcessType::Generic => "普通进程",
            ProcessType::WebService => "Web 服务",
            ProcessType::Other => "其它",
        }
    }
}

/// Web 服务地址（FR-PROC-02）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Address {
    /// tcp / http / https / udp
    pub protocol: String,
    pub host: String,
    pub port: u16,
    /// 仅 http/https 有意义
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl Address {
    /// 完整 URL，如 `http://127.0.0.1:8080/health`。
    pub fn url(&self) -> String {
        match (self.protocol.as_str(), &self.path) {
            ("http" | "https", Some(p)) => {
                let p = p.trim_start_matches('/');
                format!("{}://{}:{}/{}", self.protocol, self.host, self.port, p)
            }
            _ => format!("{}://{}:{}", self.protocol, self.host, self.port),
        }
    }

    /// 是否可用系统浏览器打开（FR-PROC-03）。
    pub fn openable(&self) -> bool {
        matches!(self.protocol.as_str(), "http" | "https")
    }
}

/// 日志源（FR-LOG-01）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogSource {
    pub name: String,
    /// 文件路径；配置了读取命令时可留空
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// 如 `journalctl -u xxx --no-pager`；输出按行解析
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// v1 支持 utf-8（默认）与 gbk
    #[serde(default = "default_encoding")]
    pub encoding: String,
    /// 行格式：json_lines / logfmt / bracket / plain
    #[serde(default = "default_log_format")]
    pub format: String,
}

fn default_encoding() -> String {
    "utf-8".into()
}
fn default_log_format() -> String {
    "plain".into()
}

/// 进程配置条目（FR-PROC-04/05/07）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProcessConfig {
    pub id: String,
    pub name: String,
    #[serde(rename = "type", default)]
    pub ptype: ProcessType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_deps_command: Option<String>,
    /// true：start_command 执行并等待退出（systemctl 类“点火即走”命令）；
    /// false（默认）：spawn 为长驻子进程并跟踪其 PID（ADR-03）。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub start_detached: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_dir: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// 系统进程模式：按进程名（子串）观察，不配置任何命令（FR-PROC-07）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observe_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<Address>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub log_sources: Vec<LogSource>,
}

impl ProcessConfig {
    /// 是否为纯观察（系统进程）模式：无任何命令且配置了观察名。
    pub fn is_observe_only(&self) -> bool {
        self.start_command.is_none()
            && self.stop_command.is_none()
            && self.check_command.is_none()
            && self.install_deps_command.is_none()
            && self.observe_name.is_some()
    }
}

/// 通用配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct General {
    /// 状态/指标刷新间隔（秒），默认 3（FR-PROC-13）
    #[serde(default = "default_refresh")]
    pub refresh_secs: u64,
    /// HTTP API 端口；0 = 由系统分配（FR-API-01），实际值启动后回写
    #[serde(default = "default_port")]
    pub http_port: u16,
    /// WebSocket 端口；0 = 由系统分配
    #[serde(default = "default_port")]
    pub ws_port: u16,
    /// 本地令牌；首次启动生成并持久化（NFR-SEC-01）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    /// skill 安装目标目录；空 = 默认 ~/.agents/skills（FR-SKILL-02）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_dir: Option<String>,
    /// 日志分页大小 50–1000，默认 100（FR-LOG-05）
    #[serde(default = "default_page_size")]
    pub log_page_size: usize,
    /// 主题模式：light / dark / system（任务 3）
    #[serde(default = "default_theme")]
    pub theme_mode: String,
    /// 界面字族；空 = 平台默认
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    /// 字号缩放 0.8–1.5，默认 1.0
    #[serde(default = "default_font_scale")]
    pub font_scale: f32,
    /// 窗口背景覆盖色（#RRGGBBAA）；None = 跟随主题
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_color: Option<String>,
    /// 系统托盘开关（默认关，任务 8；重启生效）
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tray_enabled: bool,
    /// 界面语言（任务 3）；zh-Hans 为默认
    #[serde(default = "default_lang")]
    pub lang: String,
    /// 用户选择跳过的更新版本（任务 7）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_update: Option<String>,
}

fn default_refresh() -> u64 {
    3
}
fn default_port() -> u16 {
    0
}
fn default_page_size() -> usize {
    100
}
fn default_theme() -> String {
    "system".into()
}
fn default_font_scale() -> f32 {
    1.0
}
fn default_lang() -> String {
    "zh-Hans".into()
}

impl Default for General {
    fn default() -> Self {
        Self {
            refresh_secs: default_refresh(),
            http_port: default_port(),
            ws_port: default_port(),
            auth_token: None,
            skill_dir: None,
            log_page_size: default_page_size(),
            theme_mode: default_theme(),
            font_family: None,
            font_scale: default_font_scale(),
            bg_color: None,
            tray_enabled: false,
            lang: default_lang(),
            skip_update: None,
        }
    }
}

/// 根配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub general: General,
    #[serde(default, rename = "process", skip_serializing_if = "Vec::is_empty")]
    pub processes: Vec<ProcessConfig>,
}

/// 平台标准配置目录下的 config.toml 路径。
pub fn config_path() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "cucumber")?;
    Some(dirs.config_dir().join("config.toml"))
}

impl Config {
    /// 加载；不存在则创建初始配置（含新生成的令牌）。
    pub fn load_or_create() -> Result<(Config, PathBuf), String> {
        let path = config_path().ok_or("无法定位平台配置目录")?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<(Config, PathBuf), String> {
        if path.exists() {
            let text = std::fs::read_to_string(path).map_err(|e| format!("读取配置失败：{e}"))?;
            let mut cfg: Config =
                toml::from_str(&text).map_err(|e| format!("解析配置失败：{e}"))?;
            cfg.ensure_token();
            Ok((cfg, path.to_path_buf()))
        } else {
            let mut cfg = Config::default();
            cfg.ensure_token();
            cfg.save_to(path)?;
            Ok((cfg, path.to_path_buf()))
        }
    }

    /// 生成随机令牌（首次启动）。
    fn ensure_token(&mut self) {
        if self.general.auth_token.as_deref().map(str::is_empty) != Some(false) {
            self.general.auth_token = Some(generate_token());
        }
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败：{e}"))?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| format!("序列化配置失败：{e}"))?;
        std::fs::write(path, text).map_err(|e| format!("写入配置失败：{e}"))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.save_to(path)
    }

    /// 新增或替换进程条目（按 id）。
    pub fn upsert_process(&mut self, pc: ProcessConfig) {
        match self.processes.iter().position(|p| p.id == pc.id) {
            Some(i) => self.processes[i] = pc,
            None => self.processes.push(pc),
        }
    }

    pub fn remove_process(&mut self, id: &str) -> bool {
        let before = self.processes.len();
        self.processes.retain(|p| p.id != id);
        before != self.processes.len()
    }

    pub fn find(&self, id: &str) -> Option<&ProcessConfig> {
        self.processes.iter().find(|p| p.id == id)
    }

    /// 展示用令牌（日志/UI 提示中脱敏）。
    pub fn masked_token(&self) -> String {
        match &self.general.auth_token {
            Some(t) if t.len() > 8 => format!("{}…{}", &t[..4], &t[t.len() - 4..]),
            Some(_) => "****".into(),
            None => "未配置".into(),
        }
    }
}

/// 32 字节随机数 → 64 位十六进制（令牌）。
pub fn generate_token() -> String {
    let bytes: [u8; 32] = rand::random();
    let mut s = String::with_capacity(64);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// 生成不与现有条目冲突的短 id（p1、p2…）。
pub fn next_process_id(existing: &[ProcessConfig]) -> String {
    let mut n = existing.len() + 1;
    loop {
        let id = format!("p{n}");
        if !existing.iter().any(|p| p.id == id) {
            return id;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Config {
        let mut cfg = Config::default();
        cfg.upsert_process(ProcessConfig {
            id: "p1".into(),
            name: "演示 Web".into(),
            ptype: ProcessType::WebService,
            start_command: Some("python3 -m http.server 8080".into()),
            stop_command: None,
            check_command: Some("true".into()),
            install_deps_command: None,
            start_detached: false,
            work_dir: None,
            env: [("PORT".to_string(), "8080".to_string())].into(),
            observe_name: None,
            addresses: vec![Address {
                protocol: "http".into(),
                host: "127.0.0.1".into(),
                port: 8080,
                path: Some("/health".into()),
            }],
            log_sources: vec![LogSource {
                name: "默认".into(),
                path: Some("/tmp/demo.log".into()),
                command: None,
                encoding: "utf-8".into(),
                format: "json_lines".into(),
            }],
        });
        cfg
    }

    #[test]
    fn toml_roundtrip() {
        let cfg = sample();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn token_generated_and_stable() {
        let (cfg, path) =
            Config::load_from(Path::new("/tmp/cucumber-test-token/config.toml")).unwrap();
        assert_eq!(cfg.general.auth_token.as_ref().unwrap().len(), 64);
        let (cfg2, _) = Config::load_from(&path).unwrap();
        assert_eq!(cfg.general.auth_token, cfg2.general.auth_token);
        std::fs::remove_dir_all("/tmp/cucumber-test-token").ok();
    }

    #[test]
    fn url_format() {
        let a = Address {
            protocol: "http".into(),
            host: "127.0.0.1".into(),
            port: 8080,
            path: Some("/health".into()),
        };
        assert_eq!(a.url(), "http://127.0.0.1:8080/health");
        let b = Address {
            protocol: "tcp".into(),
            path: None,
            ..a
        };
        assert_eq!(b.url(), "tcp://127.0.0.1:8080");
    }

    #[test]
    fn upsert_remove() {
        let mut cfg = sample();
        let id = next_process_id(&cfg.processes);
        let mut pc = cfg.processes[0].clone();
        pc.id = id.clone();
        pc.name = "第二个".into();
        cfg.upsert_process(pc.clone());
        assert_eq!(cfg.processes.len(), 2);
        pc.name = "改名".into();
        cfg.upsert_process(pc);
        assert_eq!(cfg.processes.len(), 2);
        assert!(cfg.remove_process(&id));
        assert_eq!(cfg.processes.len(), 1);
    }
}
