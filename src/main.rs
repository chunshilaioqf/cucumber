//! 黄瓜（cucumber）入口：CLI 解析与运行时装配。
//!
//! - （无参数）：启动 GUI
//! - `--smoke-test [秒]`：GUI 启动 N 秒后自动退出（CI 冒烟，见 docs/架构设计.md §7）
//! - `--mcp`：MCP stdio 模式（供 AI 宿主拉起，FR-API-03）
//! - `--version` / `-V`：版本信息

mod api;
mod core;
mod docs_embed;
mod platform;
mod state;
mod ui;

use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("黄瓜 cucumber {}", env!("APP_VERSION"));
        println!(
            "git {} · {} · windui {}",
            env!("GIT_SHA"),
            env!("BUILD_TARGET"),
            windui::VERSION
        );
        return;
    }
    if args.iter().any(|a| a == "--mcp") {
        std::process::exit(mcp_main());
    }
    let smoke = smoke_secs_from(&args);
    gui_main(smoke);
}

/// 解析 `--smoke-test [秒]`（缺省 5 秒）。
fn smoke_secs_from(args: &[String]) -> Option<u64> {
    let pos = args.iter().position(|a| a == "--smoke-test")?;
    Some(
        args.get(pos + 1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(5)
            .max(1),
    )
}

fn mcp_main() -> i32 {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("启动 tokio 失败：{e}");
            return 1;
        }
    };
    let (cfg, path) = match core::config::Config::load_or_create() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let state = state::AppState::new(cfg, path);
    rt.block_on(api::mcp::stdio_server(state))
}

fn gui_main(smoke: Option<u64>) {
    let (cfg, path) = match core::config::Config::load_or_create() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("配置加载失败：{e}");
            std::process::exit(1);
        }
    };
    let state = state::AppState::new(cfg, path);

    // ---- 后台 tokio 运行时（API 服务器 + 异步命令执行），独立线程 ----
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("启动 tokio 失败：{e}");
            std::process::exit(1);
        }
    };
    let handle = rt.handle().clone();
    {
        let state = state.clone();
        std::thread::spawn(move || {
            if let Err(e) = rt.block_on(api::start_all(state.clone())) {
                eprintln!("对外接口启动失败：{e}");
            }
            // 保持 runtime 常驻
            rt.block_on(std::future::pending::<()>());
        });
    }

    // ---- windui App（两段式：channel 须在 content/run 之前）----
    let mut app = windui::prelude::App::new("黄瓜 · 进程与服务管理", 1200, 780);
    // channel 闭包要访问 UiCtx，而 UiCtx 持有 Sender——用 UI 线程槽位解环
    let slot: std::rc::Rc<std::cell::RefCell<Option<ui::UiCtx>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot2 = slot.clone();
    let tx = app.channel::<ui::UiMsg>(move |ctx, msg| {
        // 克隆一份（Signal 均为 Copy 句柄，代价极小），避免 RefCell 借用跨回调存活
        let ui_owned = slot2.borrow().as_ref().cloned();
        let Some(ui) = ui_owned else { return };
        match msg {
            ui::UiMsg::Snapshot => ui.refresh(),
            ui::UiMsg::ToastOk(t) => ctx.toast_ok(t),
            ui::UiMsg::ToastErr(e) => ctx.toast_err(e),
            ui::UiMsg::LogReady(lines, hits, info, page) => {
                ui.log_lines.set(lines);
                ui.log_page.set(page);
                ui.log_info.set(format!("{info}·命中 {hits} 行"));
            }
            ui::UiMsg::HealthCheck(ok, detail) => {
                ui.diag_result.set(detail);
                if ok {
                    ctx.toast_ok("本地接口可达");
                } else {
                    ctx.toast_err("本地接口不可达");
                }
            }
        }
    });
    let uictx = ui::UiCtx::new(state.clone(), handle, tx);
    *slot.borrow_mut() = Some(uictx.clone());

    // ---- 系统采集线程（ADR-06：采集与 UI 解耦，经 channel 回投）----
    {
        let state = state.clone();
        let tx = uictx.tx.clone();
        std::thread::spawn(move || {
            let mut obs = core::sys_observer::SysObserver::new();
            loop {
                let snap = obs.refresh();
                let cfgs: Vec<core::config::ProcessConfig> = state
                    .config
                    .read()
                    .map(|c| c.processes.clone())
                    .unwrap_or_default();
                for c in &cfgs {
                    if c.observe_name.is_some() {
                        state.mgr.refresh_observe(c, &snap);
                    }
                }
                if let Ok(mut w) = state.last_snapshot.write() {
                    *w = Some(snap);
                }
                // WS 事件：dashboard 快照
                let dash = api::ops::Ops {
                    state: state.clone(),
                }
                .dashboard();
                let _ = state.events.send(dash.to_string());
                let _ = tx.send(ui::UiMsg::Snapshot);
                std::thread::sleep(Duration::from_secs(state.refresh_secs()));
            }
        });
    }

    // ---- 定时器：冒烟退出 + 日志跟随 ----
    if let Some(secs) = smoke {
        let deadline = Instant::now() + Duration::from_secs(secs);
        app = app.on_interval(Duration::from_millis(200), move |ctx| {
            if Instant::now() >= deadline {
                ctx.request_close();
            }
        });
    }
    {
        let ui = uictx.clone();
        app = app.on_interval(Duration::from_secs(1), move |_ctx| {
            ui::logs::follow_tick(&ui)
        });
    }

    // ---- 全局快捷键：Ctrl+1..7 切页，Ctrl+N 新建进程 ----
    {
        let ui = uictx.clone();
        app = app.on_shortcut(move |_ctx, ev| {
            if ev.ctrl && ev.pressed {
                if let windui::event::Key::Char(c) = ev.key {
                    if c.is_ascii_digit() && (b'1'..=b'7').contains(&(c as u8)) {
                        ui.page.set((c as u8 - b'1') as usize);
                        return true;
                    }
                    if c == 'n' || c == 'N' {
                        ui::processes::open_editor_new(&ui);
                        ui.page.set(1);
                        return true;
                    }
                }
            }
            false
        });
    }

    let root = ui::app::build(&uictx);
    app.content(root).centered().min_size(980, 620).run();
}
