//! 黄瓜（cucumber）入口：CLI 解析与运行时装配。
//! Windows 发布版使用 GUI 子系统（无控制台黑窗）；`--mcp` 模式重附父控制台。
//!
//! - （无参数）：启动 GUI
//! - `--smoke-test [秒]`：GUI 启动 N 秒后自动退出（CI 冒烟）
//! - `--mcp`：MCP stdio 模式（供 AI 宿主拉起，FR-API-03）
//! - `--version` / `-V`：版本信息

// Windows 发布版不创建控制台窗口（GUI 子系统；任务 2）
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod api;
mod core;
mod docs_embed;
mod platform;
mod state;
mod ui;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windui::prelude::*;

use crate::ui::i18n::Lang;

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
        attach_console_for_stdio();
        std::process::exit(mcp_main());
    }
    let smoke = smoke_secs_from(&args);
    gui_main(smoke);
}

/// `--mcp` 模式：GUI 子系统进程重附父控制台，恢复 stdio 行协议（任务 2）。
#[cfg(all(target_os = "windows", not(debug_assertions)))]
fn attach_console_for_stdio() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(pid: usize) -> i32;
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            sa: *const u8,
            disp: u32,
            flags: u32,
            tmpl: usize,
        ) -> *mut core::ffi::c_void;
    }
    #[link(name = "msvcrt")]
    unsafe extern "system" {
        fn _open_osfhandle(osfhandle: isize, flags: i32) -> i32;
        fn _dup2(src: i32, dst: i32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: usize = usize::MAX;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_ALL: u32 = 0x7;
    const OPEN_EXISTING: u32 = 3;
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
            let name: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
            let h = CreateFileW(
                name.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_ALL,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                0,
            );
            if !h.is_null() && h as usize != usize::MAX {
                let fd = _open_osfhandle(h as isize, 0);
                if fd != -1 {
                    _dup2(fd, 1);
                    _dup2(fd, 2);
                }
            }
        }
    }
}

#[cfg(not(all(target_os = "windows", not(debug_assertions))))]
fn attach_console_for_stdio() {}

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
    let tray_enabled = cfg.general.tray_enabled;
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
    let probe_handle = rt.handle().clone();
    {
        let state = state.clone();
        std::thread::spawn(move || {
            if let Err(e) = rt.block_on(api::start_all(state.clone())) {
                eprintln!("对外接口启动失败：{e}");
            }
            rt.block_on(std::future::pending::<()>());
        });
    }

    // ---- windui App（两段式：channel/theme/tray 须在 content/run 之前）----
    let mut app = App::new("黄瓜 · 进程与服务管理", 1200, 780);
    let theme = app.theme_handle();

    let slot: std::rc::Rc<std::cell::RefCell<Option<ui::UiCtx>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot2 = slot.clone();
    let tx = app.channel::<ui::UiMsg>(move |ctx, msg| {
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
            ui::UiMsg::TrayShow => {
                ctx.show_window();
            }
            ui::UiMsg::TrayQuit => {
                ctx.request_close();
            }
            ui::UiMsg::FontsLoaded(list) => {
                ui.font_list.set(list);
                ui.font_dialog.set(true);
            }
            ui::UiMsg::UpdateAvailable(ver, url) => {
                // 用户已跳过的版本不再提示（任务 7）
                let skipped = ui
                    .app
                    .config
                    .read()
                    .ok()
                    .and_then(|c| c.general.skip_update.clone());
                if core::update::should_prompt(&ver, skipped.as_deref()) {
                    ui.update_ver.set(ver);
                    ui.update_url.set(url);
                    ui.update_show.set(true);
                }
            }
        }
    });
    let mut uictx = ui::UiCtx::new(state.clone(), handle, tx.clone(), theme);
    // 系统托盘（任务 5）：platform::tray 跨平台实现（win/mac tray-icon；linux ksni）
    if tray_enabled {
        if let Some(bridge) = platform::tray::TrayBridge::start(tx.clone(), String::new()) {
            uictx.tray = Some(std::sync::Arc::new(bridge));
        }
    }
    *slot.borrow_mut() = Some(uictx.clone());

    // 启动时应用保存的主题/字体/背景设置（任务 3）
    ui::settings::apply_theme(&uictx);
    // ---- 端口探测任务（0.3.1，ADR-14）：每刷新周期探测全部服务地址 ----
    {
        let state = state.clone();
        probe_handle.spawn(async move {
            loop {
                let (secs, procs) = {
                    let c = state.config.read().unwrap();
                    (c.general.refresh_secs.max(1), c.processes.clone())
                };
                let result = core::port_probe::probe_all(&procs).await;
                // 写共享表 + 融合进管理器状态（供 effective_state 使用）
                if let Ok(mut m) = state.port_status.write() {
                    *m = result.clone();
                }
                let mgr = state.mgr.clone();
                for p in &procs {
                    if let Some(up) = result.get(&p.id) {
                        mgr.set_port_up(&p.id, *up);
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            }
        });
    }

    // ---- 系统采集线程（ADR-06）----
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

    // ---- 定时器：冒烟退出 / 语言桥接 / 更新检查（任务 7：启动 1 分钟后）----
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
            // 日志跟随
            ui::logs::follow_tick(&ui);
            // 托盘统计（任务 5）：周期更新菜单首项与 tooltip
            if let Some(tb) = ui.tray.as_ref() {
                let s = ui.tray_stats.with(|v| v.clone());
                tb.set_stats(&s);
            }
            // 语言下拉桥接（dropdown 无回调；变化即应用）
            let idx = ui.lang_sel.get();
            if let Some(l) = Lang::ALL.get(idx) {
                if *l != ui.lang.get() {
                    ui::set_lang(&ui, *l);
                }
            }
            // 字号滑条桥接：norm 与 scale 不一致 → 应用（由用户拖动引发的那一侧为准）
            let want = 0.8 + ui.font_norm.get().clamp(0.0, 1.0) * 0.7;
            if (want - ui.font_scale.get()).abs() > 0.004 {
                ui.font_scale.set(want);
                ui.font_pct
                    .set(format!("{}%", (want * 100.0).round() as i32));
                ui::settings::apply_theme(&ui);
                ui::settings::persist(&ui);
            }
        });
    }
    {
        let ui = uictx.clone();
        let checked = std::sync::Arc::new(AtomicBool::new(false));
        app = app.on_interval(Duration::from_secs(5), move |_ctx| {
            // 启动满 60 秒且未检查过 → 检查更新（只查一次）
            if !checked.swap(true, Ordering::SeqCst) {
                let (rt, tx) = (ui.rt.clone(), ui.tx.clone());
                rt.spawn(async move {
                    if let core::update::UpdateCheck::Available { version, url } =
                        core::update::check_latest().await
                    {
                        let _ = tx.send(ui::UiMsg::UpdateAvailable(version, url));
                    }
                });
            }
        });
    }

    // ---- 跟随系统主题（任务 3：mode == system 时生效）----
    {
        let ui = uictx.clone();
        app = app.on_system_theme_changed(move |_ctx, is_dark| {
            if ui.theme_mode.get() == 2 {
                ui.theme.update(|t| {
                    let f = t.font_family.clone();
                    let scale = t.metrics.font_md / 14.0;
                    *t = if is_dark {
                        Theme::dark()
                    } else {
                        Theme::default()
                    };
                    t.font_family = f;
                    t.metrics.font_xs = (12.0 * scale).max(9.0);
                    t.metrics.font_sm = (13.0 * scale).max(10.0);
                    t.metrics.font_md = (14.0 * scale).max(10.0);
                    t.metrics.font_lg = (16.0 * scale).max(11.0);
                    t.metrics.font_xl = (20.0 * scale).max(12.0);
                    if ui.bg_use.get() {
                        t.palette.bg = ui.bg_color.get();
                    }
                });
            }
        });
    }

    // ---- 全局快捷键：Ctrl+1..8 切页，Ctrl+N 新建进程 ----
    {
        let ui = uictx.clone();
        app = app.on_shortcut(move |_ctx, ev| {
            if ev.ctrl && ev.pressed {
                if let windui::event::Key::Char(c) = ev.key {
                    if c.is_ascii_digit() && (b'1'..=b'8').contains(&(c as u8)) {
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
    let mut app2 = app.centered().min_size(980, 620);
    // 窗口/Dock 图标（任务 2）
    if let Some(icon) = windui::icon::WindowIcon::from_rgba(
        128,
        128,
        include_bytes!("../assets/icon_128.rgba").to_vec(),
    ) {
        app2 = app2.icon(icon);
    }
    app2.content(root).run();
}
