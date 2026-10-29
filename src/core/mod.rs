//! core：与 UI 无关的业务层（配置、进程管理、系统观察、日志、skill）。

pub mod config;
pub mod gpu;
pub mod log_index;
pub mod log_parser;
pub mod port_probe;
pub mod process_manager;
pub mod skill_installer;
pub mod sys_observer;
pub mod update;
pub mod updater;
