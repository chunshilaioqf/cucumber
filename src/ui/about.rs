//! 帮助中心 / 关于本软件（FR-HELP-01/02）：版本、环境、快速诊断、常见问题。

use windui::prelude::*;

use super::kit;
use super::{UiCtx, UiMsg};

pub fn page(ui: &UiCtx) -> Element {
    let version = Element::card(
        "关于黄瓜（cucumber）",
        Element::col()
            .spacing(4)
            .child(
                Element::label("黄瓜 · 跨平台本地进程与服务统一管理工具")
                    .font_weight(600)
                    .font_size(14.0),
            )
            .child(
                Element::label(format!(
                    "版本 {}（git {}）",
                    env!("APP_VERSION"),
                    env!("GIT_SHA")
                ))
                .font_size(12.5),
            )
            .child(
                Element::label(format!(
                    "构建目标 {} · windui {}",
                    env!("BUILD_TARGET"),
                    windui::VERSION
                ))
                .font_size(12.5),
            )
            .child(
                Element::label(
                    "GUI 框架 windui（MIT OR Apache-2.0）· HTTP/WS axum · 系统信息 sysinfo",
                )
                .fg(kit::colors::GRAY)
                .font_size(12.0),
            )
            .child(
                Element::hstack()
                    .spacing(6)
                    .child(
                        Element::label(format!(
                            "{}：",
                            crate::ui::i18n::tr(ui.lang.get(), "h.repo")
                        ))
                        .font_size(12.5),
                    )
                    .child(Element::link(crate::core::update::REPO_URL).on_click({
                        let ui = ui.clone();
                        move |_| {
                            let (rt, tx) = (ui.rt.clone(), ui.tx.clone());
                            rt.spawn(async move {
                                let _ = crate::platform::browser::open_url(
                                    crate::core::update::REPO_URL,
                                )
                                .await;
                                let _ = tx.send(crate::ui::UiMsg::ToastOk("已打开仓库页面".into()));
                            });
                        }
                    })),
            )
            .child(
                Element::label(format!(
                    "{}：chunshilaioqf（贡献者见仓库提交历史）",
                    crate::ui::i18n::tr(ui.lang.get(), "h.author")
                ))
                .font_size(12.5),
            ),
    );

    // 快速诊断（FR-HELP-02）
    let diag = Element::card(
        "快速诊断",
        Element::col()
            .spacing(8)
            .width_match()
            .child(Element::button("检查本地接口可达性").on_click({
                let app = ui.app.clone();
                let (rt, tx) = (ui.rt.clone(), ui.tx.clone());
                move |_| {
                    let (app2, tx2) = (app.clone(), tx.clone());
                    rt.spawn(async move {
                        let port = app2.http_port.load(std::sync::atomic::Ordering::Relaxed);
                        if port == 0 {
                            let _ = tx2.send(UiMsg::HealthCheck(false, "HTTP 服务未启动".into()));
                            return;
                        }
                        let url = format!("http://127.0.0.1:{port}/api/health");
                        match reqwest_lite(&url).await {
                            Ok(body) => {
                                let _ = tx2.send(UiMsg::HealthCheck(
                                    body.contains("cucumber"),
                                    format!("✓ {url} → {body}"),
                                ));
                            }
                            Err(e) => {
                                let _ =
                                    tx2.send(UiMsg::HealthCheck(false, format!("✗ {url}：{e}")));
                            }
                        }
                    });
                }
            }))
            .child(Element::label_signal(ui.diag_result).font_size(12.5)),
    );

    let env_info = Element::card(
        "环境信息",
        Element::col()
            .spacing(4)
            .child(Element::label(format!("平台：{}", std::env::consts::OS)).font_size(12.5))
            .child(Element::label(format!("渲染后端：{}", backend_name())).font_size(12.5))
            .child(
                Element::label(format!("配置文件：{}", ui.app.config_path.display()))
                    .font_size(12.5),
            )
            .child(
                Element::label("GPU 指标：sysinfo 尚未提供 GPU 传感器接口，仪表盘以占位卡显示（任务 5 已知限制）")
                    .fg(kit::colors::GRAY)
                    .font_size(11.5),
            ),
    );

    let faq = Element::card(
        "常见问题",
        Element::rich(kit::markdown_doc(
            "- **进程启动失败？** 「进程」页点「编辑」检查命令；命令输出保留在 get_process 的 recent_output（API 可查）。\n\
             - **日志高亮不生效？** 检查日志源格式配置（json_lines/logfmt/bracket/plain）；plain 按整词嗅探级别。\n\
             - **Wayland 下启动？** 默认经 XWayland；`WINDUI_BACKEND=wayland` 启用原生后端（全局热键不可用）。\n\
             - **停止不了进程？** 优先配置 stop_command；未配置时对子进程 SIGTERM→5 秒→SIGKILL，Windows 用 taskkill。\n\
             - **接口 401？** 令牌见「对外接口」页，或读配置文件 auth_token 字段。",
        )),
    );

    Element::scroll().child(
        Element::col()
            .fill()
            .padding(16)
            .spacing(14)
            .child(version)
            .child(diag)
            .child(env_info)
            .child(faq),
    )
}

fn backend_name() -> &'static str {
    match std::env::var("WINDUI_BACKEND").as_deref() {
        Ok("wayland") => "Linux · Wayland（原生）",
        Ok("x11") => "Linux · X11",
        Ok(_) => "自定义",
        Err(_) => {
            if cfg!(target_os = "windows") {
                "Windows · Win32 + DirectWrite"
            } else if cfg!(target_os = "macos") {
                "macOS · Cocoa + CoreText"
            } else {
                "Linux · X11（默认；WINDUI_BACKEND=wayland 切原生）"
            }
        }
    }
}

/// 轻量 HTTP GET（不引 reqwest：走 tokio 原生 TCP 手写最小 HTTP/1.0 请求）。
async fn reqwest_lite(url: &str) -> Result<String, String> {
    let rest = url.strip_prefix("http://").ok_or("仅支持 http")?;
    let (host, path) = match rest.split_once('/') {
        Some((h, p)) => (h, format!("/{p}")),
        None => (rest, "/".into()),
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(host)
        .await
        .map_err(|e| e.to_string())?;
    let req = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .await
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf);
    // 分离 header / body
    Ok(text.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
}
