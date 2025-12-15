use anyhow::Result;
use crossbeam_channel::Sender;
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
pub fn init_tray(exit_tx: Sender<()>) -> Result<()> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/panda.ico");

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
    let quit_i = MenuItem::new("退出", true, None);
    tray_menu.append_items(&[
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
                let icon = load_icon(std::path::Path::new(path));

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
                }
            }

            _ => {}
        }
    })
}

/// 加载自定义托盘图标
fn load_icon(path: &std::path::Path) -> tray_icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
