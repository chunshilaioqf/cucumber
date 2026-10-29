//! 跨平台系统托盘（任务 5，ADR-15）：
//! - Windows / macOS：`tray-icon`（原生 Shell_NotifyIcon / NSStatusItem）
//! - Linux：`ksni`（DBus StatusNotifierItem，纯 Rust、零 GTK/C 依赖，X11/Wayland 通用）
//!
//! 菜单事件统一转发为 `UiMsg` 回 UI 线程处理。

use crate::ui::UiMsg;

#[cfg(not(target_os = "linux"))]
mod native {
    use super::*;

    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

    #[derive(Clone)]
    pub struct TrayBridge {
        icon: TrayIcon,
        stats_item: MenuItem,
    }

    impl TrayBridge {
        pub fn start(to_ui: windui::prelude::Sender<UiMsg>, stats: String) -> Option<Self> {
            let show = MenuItem::with_id("show", "显示窗口", true, None);
            let quit = MenuItem::with_id("quit", "退出", true, None);
            let stats_item = MenuItem::with_id("stats", &stats, false, None);
            let menu = Menu::new();
            let _ = menu.append(&stats_item);
            let _ = menu.append(&tray_icon::menu::PredefinedMenuItem::separator());
            let _ = menu.append(&show);
            let _ = menu.append(&tray_icon::menu::PredefinedMenuItem::separator());
            let _ = menu.append(&quit);

            let rgba = include_bytes!("../../assets/icon_64.rgba").to_vec();
            let icon = tray_icon::Icon::from_rgba(rgba, 64, 64).ok()?;
            let icon = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip(format!("黄瓜 · {stats}"))
                .with_icon(icon)
                .build()
                .ok()?;

            let tx = to_ui.clone();
            std::thread::spawn(move || {
                let rx = MenuEvent::receiver();
                while let Ok(ev) = rx.recv() {
                    match ev.id.0.as_str() {
                        "show" => {
                            let _ = tx.send(UiMsg::TrayShow);
                        }
                        "quit" => {
                            let _ = tx.send(UiMsg::TrayQuit);
                        }
                        _ => {}
                    }
                }
            });
            let tx = to_ui.clone();
            std::thread::spawn(move || {
                let rx = TrayIconEvent::receiver();
                while let Ok(ev) = rx.recv() {
                    if let tray_icon::TrayIconEvent::Click {
                        button: tray_icon::MouseButton::Left,
                        button_state: tray_icon::MouseButtonState::Up,
                        ..
                    } = ev
                    {
                        let _ = tx.send(UiMsg::TrayShow);
                    }
                }
            });

            Some(Self { icon, stats_item })
        }

        pub fn set_stats(&self, text: &str) {
            self.stats_item.set_text(text);
            let _ = self.icon.set_tooltip(Some(format!("黄瓜 · {text}")));
        }
    }
}

#[cfg(target_os = "linux")]
mod native {
    use super::*;
    use ksni::blocking::TrayMethods as _;
    use std::sync::{Arc, Mutex};

    struct CucumberTray {
        to_ui: windui::prelude::Sender<UiMsg>,
        stats: Arc<Mutex<String>>,
    }

    impl ksni::Tray for CucumberTray {
        fn id(&self) -> String {
            "cucumber-tray".into()
        }

        fn title(&self) -> String {
            format!("黄瓜 · {}", self.stats.lock().unwrap().clone())
        }

        fn tool_tip(&self) -> ksni::ToolTip {
            ksni::ToolTip {
                title: "黄瓜 · 进程与服务管理".into(),
                description: self.stats.lock().unwrap().clone(),
                ..Default::default()
            }
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::*;
            let stats = self.stats.lock().unwrap().clone();
            vec![
                StandardItem {
                    label: stats,
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: "显示窗口".into(),
                    activate: Box::new(|t: &mut Self| {
                        let _ = t.to_ui.send(UiMsg::TrayShow);
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: "退出".into(),
                    activate: Box::new(|t: &mut Self| {
                        let _ = t.to_ui.send(UiMsg::TrayQuit);
                    }),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }

    pub struct TrayBridge {
        stats: Arc<Mutex<String>>,
        handle: ksni::blocking::Handle<CucumberTray>,
    }

    impl TrayBridge {
        pub fn start(to_ui: windui::prelude::Sender<UiMsg>, stats: String) -> Option<Self> {
            let stats_arc = Arc::new(Mutex::new(stats));
            let handle = CucumberTray {
                to_ui,
                stats: stats_arc.clone(),
            }
            .spawn()
            .ok()?;
            Some(Self {
                stats: stats_arc,
                handle,
            })
        }

        pub fn set_stats(&self, text: &str) {
            *self.stats.lock().unwrap() = text.to_string();
            let _ = self.handle.update(|_t: &mut CucumberTray| ());
        }
    }
}

pub use native::TrayBridge;
