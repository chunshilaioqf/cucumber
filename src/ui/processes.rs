//! 进程页（FR-PROC-10~13）：列表 CRUD、命令执行入口、编辑与删除对话框。

use std::collections::BTreeMap;

use windui::prelude::*;

use super::kit;
use super::{spawn_op, ProcRow, UiCtx, UiMsg};
use crate::core::config::{Address, LogSource, ProcessConfig, ProcessType};

pub fn page(ui_ref: &UiCtx) -> Element {
    let ui = ui_ref.clone();
    let add_btn = Element::button(ui.t.c_add).on_click({
        let ui = ui.clone();
        move |_| open_editor_new(&ui)
    });

    let ui_for_rows = ui.clone();
    let list = Element::list_signal(
        ui.rows,
        |r: &ProcRow| r.id.clone(),
        move |r: ProcRow| row(&ui_for_rows, r),
    )
    .weight(1.0);

    let empty_hint = Element::label_signal(ui.t.p_empty)
        .fg_role(Role::TextMuted)
        .visible_when(move || ui.rows.with(|r| r.is_empty()));

    let body = Element::col()
        .fill()
        .padding_xy(16, 8)
        .spacing(10)
        .child(
            Element::hstack()
                .width_match()
                .child(add_btn)
                .child(Element::flex_spacer())
                .child(
                    Element::label_signal(ui.t.p_auto)
                        .fg_role(Role::TextMuted)
                        .font_size(12.0),
                ),
        )
        .child(empty_hint)
        .child(list);

    Element::stack()
        .child(body)
        .child(edit_dialog(&ui))
        .child(delete_dialog(&ui))
}

fn row(ui: &UiCtx, r: ProcRow) -> Element {
    let state_color = if r.running {
        kit::colors::GREEN
    } else if r.state_label.contains("异常") {
        kit::colors::RED
    } else {
        kit::colors::GRAY
    };
    let busy = r.state_label.ends_with("中") && !r.running; // 启动中/停止中
    let mut actions = Element::hstack().spacing(6);

    if r.has_start {
        actions = actions.child(
            Element::button("启动")
                .small()
                .on_click({
                    let (ui2, id2, name2) = (ui.clone(), r.id.clone(), r.name.clone());
                    let (tx, rt) = (ui.tx.clone(), ui.rt.clone());
                    move |_| {
                        let (ui3, id3) = (ui2.clone(), id2.clone());
                        spawn_op(
                            tx.clone(),
                            &rt,
                            format!("已发起启动：{name2}"),
                            move || {
                                let state = ui3.app.clone();
                                let id4 = id3.clone();
                                async move {
                                    crate::api::ops::Ops { state }.start(&id4).await.map(|_| ())
                                }
                            },
                        );
                    }
                })
                .enabled_when(move || !(r.running || busy)),
        );
    }
    if r.has_stop {
        actions =
            actions.child(
                Element::button("停止")
                    .small()
                    .outline()
                    .on_click({
                        let (ui2, id2, name2) = (ui.clone(), r.id.clone(), r.name.clone());
                        let (tx, rt) = (ui.tx.clone(), ui.rt.clone());
                        move |_| {
                            let (ui3, id3) = (ui2.clone(), id2.clone());
                            spawn_op(tx.clone(), &rt, format!("已停止：{name2}"), move || {
                                let state = ui3.app.clone();
                                let id4 = id3.clone();
                                async move {
                                    crate::api::ops::Ops { state }.stop(&id4).await.map(|_| ())
                                }
                            });
                        }
                    })
                    .enabled_when(move || !busy),
            );
    }
    if r.has_check {
        actions = actions.child(
            Element::button("检查")
                .small()
                .outline()
                .on_click({
                    let (ui2, id2) = (ui.clone(), r.id.clone());
                    let (tx, rt) = (ui.tx.clone(), ui.rt.clone());
                    move |_| {
                        let (ui3, id3) = (ui2.clone(), id2.clone());
                        spawn_op(
                            tx.clone(),
                            &rt,
                            "检查完成，退出码与输出已记录".into(),
                            move || {
                                let state = ui3.app.clone();
                                let id4 = id3.clone();
                                async move {
                                    crate::api::ops::Ops { state }.check(&id4).await.map(|_| ())
                                }
                            },
                        );
                    }
                })
                .enabled_when(move || !busy),
        );
    }
    actions = actions
        .child(Element::button("编辑").small().neutral().on_click({
            let (ui2, id2) = (ui.clone(), r.id.clone());
            move |_| open_editor_edit(&ui2, &id2)
        }))
        .child(
            Element::button("删除")
                .small()
                .danger()
                .outline()
                .on_click({
                    let (ui2, id2, name2) = (ui.clone(), r.id.clone(), r.name.clone());
                    move |_| {
                        ui2.del_target.set(Some((id2.clone(), name2.clone())));
                        ui2.del_show.set(true);
                    }
                }),
        );

    Element::row()
        .width_match()
        .cross(Align::Center)
        .margin_xy(0, 3)
        .padding_xy(10, 8)
        .bg_role(Role::Surface)
        .corner(8.0)
        .border_role(Role::Border, 1)
        .spacing(10)
        .animate_item()
        .child(
            Element::col()
                .width(230)
                .spacing(2)
                .child(Element::label(r.name.clone()).font_weight(600))
                .child(
                    Element::hstack()
                        .spacing(6)
                        .child(Element::badge(r.type_label.clone()))
                        .child(if r.observe_only {
                            Element::badge("系统观察")
                        } else {
                            Element::label("").width(0)
                        }),
                ),
        )
        .child(
            Element::col()
                .width(230)
                .spacing(2)
                .child(
                    Element::label(format!("{} · PID {}", r.state_label, r.pid_text))
                        .fg(state_color),
                )
                .child(
                    Element::label(format!("CPU/内存 {}", r.res_text))
                        .fg_role(Role::TextMuted)
                        .font_size(11.5),
                ),
        )
        .child(Element::flex_spacer())
        .child(actions)
}

// ---------- 编辑对话框 ----------

/// 新建（菜单「添加进程」与页面按钮共用）。
pub fn open_editor_new(ui: &UiCtx) {
    let e = &ui.edit;
    e.editing_id.set(String::new());
    e.name.set(String::new());
    e.ptype.set(0);
    e.observe_name.set(String::new());
    e.start_cmd.set(String::new());
    e.stop_cmd.set(String::new());
    e.check_cmd.set(String::new());
    e.install_cmd.set(String::new());
    e.detached.set(false);
    e.work_dir.set(String::new());
    e.env_text.set(String::new());
    e.addrs_text.set(String::new());
    e.logs_text.set(String::new());
    e.show.set(true);
}

pub fn open_editor_edit(ui: &UiCtx, id: &str) {
    let cfg = ui.app.config.read().unwrap();
    let Some(p) = cfg.find(id) else { return };
    let e = &ui.edit;
    e.editing_id.set(p.id.clone());
    e.name.set(p.name.clone());
    e.ptype.set(match p.ptype {
        ProcessType::Generic => 0,
        ProcessType::WebService => 1,
        ProcessType::Other => 2,
    });
    e.observe_name
        .set(p.observe_name.clone().unwrap_or_default());
    e.start_cmd.set(p.start_command.clone().unwrap_or_default());
    e.stop_cmd.set(p.stop_command.clone().unwrap_or_default());
    e.check_cmd.set(p.check_command.clone().unwrap_or_default());
    e.install_cmd
        .set(p.install_deps_command.clone().unwrap_or_default());
    e.detached.set(p.start_detached);
    e.work_dir.set(p.work_dir.clone().unwrap_or_default());
    e.env_text.set(
        p.env
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    e.addrs_text.set(
        p.addresses
            .iter()
            .map(|a| a.url())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    e.logs_text.set(
        p.log_sources
            .iter()
            .map(|s| {
                format!(
                    "{} | {} | {} | {}",
                    s.name,
                    s.path.clone().unwrap_or_default(),
                    s.command.clone().unwrap_or_default(),
                    s.format
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
    e.show.set(true);
}

/// 解析多行地址文本 → Address 列表（坏行跳过并计数）。
pub fn parse_addrs(text: &str) -> (Vec<Address>, usize) {
    let mut out = Vec::new();
    let mut bad = 0;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        match parse_addr(t) {
            Some(a) => out.push(a),
            None => bad += 1,
        }
    }
    (out, bad)
}

/// `proto://host:port[/path]` 或 `host:port`（默认 http）。
fn parse_addr(t: &str) -> Option<Address> {
    let (proto, rest) = match t.split_once("://") {
        Some((p, r)) => (p.to_string(), r),
        None => ("http".to_string(), t),
    };
    let (hostport, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, Some(format!("/{}", p.trim_end_matches('/')))),
        None => (rest, None),
    };
    let (host, port) = hostport.rsplit_once(':')?;
    let port: u16 = port.parse().ok()?;
    if host.is_empty() || !(1..=65535).contains(&port) {
        return None;
    }
    Some(Address {
        protocol: proto,
        host: host.into(),
        port,
        path,
    })
}

/// 解析日志源多行文本：`名称 | 路径 | 命令 | 格式`。
fn parse_log_sources(text: &str) -> Vec<LogSource> {
    text.lines()
        .filter_map(|l| {
            let t = l.trim();
            if t.is_empty() {
                return None;
            }
            let parts: Vec<&str> = t.split('|').map(str::trim).collect();
            let name = parts.first()?.to_string();
            if name.is_empty() {
                return None;
            }
            Some(LogSource {
                name,
                path: parts
                    .get(1)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
                command: parts
                    .get(2)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
                encoding: "utf-8".into(),
                format: parts.get(3).unwrap_or(&"plain").to_string(),
            })
        })
        .collect()
}

fn save(ui: &UiCtx) {
    let e = &ui.edit;
    let ptype = match e.ptype.get() {
        1 => ProcessType::WebService,
        2 => ProcessType::Other,
        _ => ProcessType::Generic,
    };
    let env: BTreeMap<String, String> = e
        .env_text
        .get()
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() {
                return None;
            }
            l.split_once('=')
                .map(|(k, v)| (k.trim().into(), v.trim().into()))
        })
        .collect();
    let (addrs, bad) = parse_addrs(&e.addrs_text.get());
    if bad > 0 {
        let _ = ui.tx.send(UiMsg::ToastErr(format!(
            "{bad} 行服务地址格式有误（应为 proto://host:port[/path]），未保存"
        )));
        return;
    }
    let cfg = ProcessConfig {
        // 空则由 upsert 分配新 id
        id: e.editing_id.get(),
        name: e.name.get().trim().to_string(),
        ptype,
        start_command: opt(&e.start_cmd.get()),
        stop_command: opt(&e.stop_cmd.get()),
        check_command: opt(&e.check_cmd.get()),
        install_deps_command: opt(&e.install_cmd.get()),
        start_detached: e.detached.get(),
        work_dir: opt(&e.work_dir.get()),
        env,
        observe_name: opt(&e.observe_name.get()),
        addresses: addrs,
        log_sources: parse_log_sources(&e.logs_text.get()),
    };
    e.show.set(false);
    let ui2 = ui.clone();
    let (tx2, rt2) = (ui.tx.clone(), ui.rt.clone());
    spawn_op(tx2, &rt2, "已保存进程配置".into(), move || {
        let state = ui2.app.clone();
        let cfg = cfg.clone();
        async move {
            crate::api::ops::Ops { state }
                .upsert_process(cfg)
                .map(|_| ())
        }
    });
    // 保存后刷新列表（upsert 在后台完成，Snapshot 兜底）
    let tx = ui.tx.clone();
    let rt = ui.rt.clone();
    rt.spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let _ = tx.send(UiMsg::Snapshot);
    });
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn edit_dialog(ui: &UiCtx) -> Element {
    let e = &ui.edit;
    let f = |label: &str, sig: &Signal<String>, placeholder: &str, w: i32| {
        Element::field(label, Element::text_input(*sig, placeholder).width(w))
    };
    let cmd_group = Element::collapsible(
        "命令配置",
        signal(true),
        Element::col()
            .width_match()
            .spacing(8)
            .child(f(
                "启动命令",
                &e.start_cmd,
                "如：python3 -m http.server 8080",
                460,
            ))
            .child(f("停止命令", &e.stop_cmd, "缺省时对子进程发终止信号", 460))
            .child(f("检查命令", &e.check_cmd, "退出码 0 = 健康", 460))
            .child(f(
                "安装依赖",
                &e.install_cmd,
                "如：pip install -r requirements.txt",
                460,
            ))
            .child(Element::setting_row(
                "启动命令为“点火即走”（如 systemctl start，执行后即返回）",
                Element::switch(e.detached),
            ))
            .child(f("工作目录", &e.work_dir, "留空继承本工具目录", 460))
            .child(Element::field(
                "环境变量",
                Element::text_input(e.env_text, "每行一条 KEY=VALUE")
                    .multiline()
                    .width(460)
                    .height(72),
            )),
    );

    let addr_group = Element::collapsible(
        "服务地址（Web 服务类型）",
        signal(true),
        Element::col()
            .width_match()
            .spacing(6)
            .child(
                Element::text_input(
                    e.addrs_text,
                    "每行一条 proto://host:port[/path]，如 http://127.0.0.1:8080/health",
                )
                .multiline()
                .width(460)
                .height(72),
            )
            .child(
                Element::label(
                    "协议：tcp / http / https / udp；http/https 可在「所有服务」页一键打开",
                )
                .fg_role(Role::TextMuted)
                .font_size(12.0),
            ),
    );

    let logs_group = Element::collapsible(
        "日志源",
        signal(true),
        Element::col()
            .width_match()
            .spacing(6)
            .child(
                Element::text_input(e.logs_text, "每行：名称 | 文件路径 | 读取命令 | 格式")
                    .multiline()
                    .width(460)
                    .height(72),
            )
            .child(
                Element::label("格式：json_lines / logfmt / bracket / plain；路径与命令至少填一项")
                    .fg_role(Role::TextMuted)
                    .font_size(12.0),
            ),
    );

    let body = Element::scroll().child(
        Element::col()
            .width_match()
            .spacing(8)
            .padding_xy(0, 4)
            .child(f("名称", &e.name, "如：本地 API 服务", 320))
            .child(Element::field(
                "类型",
                Element::dropdown(vec!["普通进程", "Web 服务", "其它"], e.ptype).width(160),
            ))
            .child(f(
                "系统观察名",
                &e.observe_name,
                "仅观察模式：按进程名子串（如 nginx）",
                320,
            ))
            .child(cmd_group)
            .child(addr_group)
            .child(logs_group),
    );

    let footer = Element::hstack()
        .spacing(10)
        .child(Element::button("取消").neutral().on_click({
            let e = ui.edit.clone();
            move |_| e.show.set(false)
        }))
        .child(Element::button("保存").on_click({
            let ui = ui.clone();
            move |_| save(&ui)
        }));

    Element::dialog_panel(
        e.show,
        "进程配置",
        660,
        {
            let e = ui.edit.clone();
            move |_ctx| e.show.set(false)
        },
        body,
        footer,
    )
}

// ---------- 删除确认（FR-PROC-12） ----------

fn delete_dialog(ui: &UiCtx) -> Element {
    let warn = Element::label_signal(ui.t.p_delwarn)
        .fg(kit::colors::ORANGE)
        .font_size(12.5)
        .visible_when({
            let ui = ui.clone();
            move || {
                let Some((id, _)) = ui.del_target.with(|t| t.clone()) else {
                    return false;
                };
                ui.rows
                    .with(|rows| rows.iter().any(|r| r.id == id && r.running))
            }
        });

    Element::dialog(
        ui.del_show,
        Element::col()
            .width(380)
            .spacing(12)
            .child(
                Element::label("删除进程配置")
                    .font_weight(600)
                    .font_size(15.0),
            )
            .child(warn)
            .child(
                Element::hstack()
                    .spacing(10)
                    .child(Element::button("取消").neutral().on_click({
                        let ui = ui.clone();
                        move |_| {
                            ui.del_show.set(false);
                            ui.del_target.set(None);
                        }
                    }))
                    .child(Element::button("确认删除").danger().on_click({
                        let ui = ui.clone();
                        move |_| {
                            let Some((id, _)) = ui.del_target.with(|t| t.clone()) else {
                                return;
                            };
                            ui.del_show.set(false);
                            ui.del_target.set(None);
                            let ui2 = ui.clone();
                            let (tx3, rt3) = (ui.tx.clone(), ui.rt.clone());
                            spawn_op(tx3, &rt3, "已删除进程配置".into(), move || {
                                let state = ui2.app.clone();
                                let id2 = id.clone();
                                async move {
                                    crate::api::ops::Ops { state }
                                        .delete_process(&id2)
                                        .map(|_| ())
                                }
                            });
                            let tx = ui.tx.clone();
                            let rt = ui.rt.clone();
                            rt.spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                                let _ = tx.send(UiMsg::Snapshot);
                            });
                        }
                    })),
            ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addr_parsing() {
        let (a, bad) = parse_addrs("http://127.0.0.1:8080/health\n127.0.0.1:9000\nbad");
        assert_eq!(bad, 1);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].protocol, "http");
        assert_eq!(a[0].path.as_deref(), Some("/health"));
        assert_eq!(a[1].protocol, "http");
        assert_eq!(a[1].port, 9000);
        assert_eq!(a[1].path, None);
    }

    #[test]
    fn addr_invalid() {
        assert!(parse_addr("http://:8080").is_none());
        assert!(parse_addr("http://host:notaport").is_none());
        assert!(parse_addr("host:0").is_none());
    }

    #[test]
    fn log_src_parsing() {
        let s = parse_log_sources(
            "app | /var/log/a.log | | json_lines\njournal | | journalctl -u a | plain",
        );
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].path.as_deref(), Some("/var/log/a.log"));
        assert!(s[0].command.is_none());
        assert_eq!(s[1].command.as_deref(), Some("journalctl -u a"));
        assert!(s[1].path.is_none());
        assert_eq!(s[1].format, "plain");
    }
}
