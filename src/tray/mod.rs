use anyhow::Result;
use crate::AppShared;
use crossbeam_channel::Sender;
use std::sync::Arc;
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIconBuilder, TrayIconEvent};
#[derive(Debug)]
enum UserEvent {
    TrayIconEvent(TrayIconEvent),
    MenuEvent(MenuEvent),
}
/// 初始化系统托盘
pub fn init_tray(exit_tx: Sender<()>, shared: Arc<AppShared>) -> Result<()> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // set a tray event handler that forwards the event and wakes up the event loop
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        proxy
            .send_event(UserEvent::TrayIconEvent(event))
            .expect("TrayIconEvent接收失败");
    }));
    // set a menu event handler that forwards the event and wakes up the event loop
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        proxy
            .send_event(UserEvent::MenuEvent(event))
            .expect("MenuEvent接收失败");
    }));

    // 创建菜单
    let tray_menu = Menu::new();
    let config_i = MenuItem::new("按键配置", true, None);
    let quit_i = MenuItem::new("退出", true, None);
    tray_menu.append_items(&[
        &config_i,
        &PredefinedMenuItem::separator(),
        &quit_i,
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::about(
            Some("关于nuna"),
            Some(AboutMetadata {
                name: Some("nuna小程序".to_string()),
                comments: Some("nuna小程序，增强CapsLock".to_string()),
                authors: Some(vec!["Nuna".to_string()]),
                short_version: Some("1.0".to_string()),
                ..Default::default()
            }),
        ),
    ])?;

    let mut tray_icon = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(tao::event::StartCause::Init) => {
                let icon = load_icon();

                // We create the icon once the event loop is actually running
                // to prevent issues like https://github.com/tauri-apps/tray-icon/issues/90
                tray_icon = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(tray_menu.clone()))
                        .with_tooltip("nuna - CapsLock增强小工具")
                        .with_icon(icon)
                        .build()
                        .unwrap(),
                );
            }

            Event::UserEvent(UserEvent::TrayIconEvent(event)) => {
                log::debug!("{event:?}");
            }

            Event::UserEvent(UserEvent::MenuEvent(event)) => {
                log::debug!("{event:?}");
                if event.id == quit_i.id() {
                    tray_icon.take();
                    exit_tx.send(()).unwrap();
                    *control_flow = ControlFlow::Exit;
                } else if event.id == config_i.id() {
                    // 打开按键映射配置窗口（独立线程，重复点击不会重复开窗）
                    crate::config_ui::open(shared.clone());
                }
            }

            _ => {}
        }
    })
}

/// 加载自定义托盘图标
/// 图标内嵌进二进制，避免依赖编译期绝对路径（CI 上编译的 CARGO_MANIFEST_DIR
/// 在用户机器上不存在，曾导致托盘初始化 panic 后整个程序 0xC0000409 闪退）
fn load_icon() -> tray_icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(include_bytes!("../../assets/panda.ico"))
            .expect("Failed to decode embedded icon")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
