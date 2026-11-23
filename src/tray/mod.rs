use anyhow::Result;
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{AboutMetadata, CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIconBuilder, TrayIconEvent};

/// 关于对话框里的版本说明。
/// muda 在 Windows 上把 comments 渲染为版本行之后的独立多行文本块
const VERSION_NOTES: &str = "CapsLock 增强小工具

版本说明：
v0.1.1
· 改用系统键盘钩子（WH_KEYBOARD_LL）：免驱动、免管理员、解压即用
· 修复 USB 鼠标热插拔后失灵的问题（v0.1.0 打包驱动的缺陷）
· 托盘新增「开机启动」开关（写入当前用户注册表，可随时勾选切换）
· v0.1.0 老用户：托盘菜单「清理驱动残留」一键移除旧驱动过滤器

v0.1.0
· 首个版本：CapsLock 组合键映射（Interception 驱动方案）";

#[derive(Debug)]
enum UserEvent {
    TrayIconEvent(TrayIconEvent),
    MenuEvent(MenuEvent),
}
/// 初始化系统托盘（阻塞运行事件循环，用户点退出后返回）
pub fn init_tray() -> Result<()> {
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
    // 勾选态与注册表实际状态同步；切换后若写注册表失败会回滚勾选
    let autostart_i = CheckMenuItem::with_id(
        "autostart",
        "开机启动",
        true,
        crate::autostart::is_enabled(),
        None,
    );
    let cleanup_i = MenuItem::with_id("cleanup", "清理驱动残留", true, None);
    let quit_i = MenuItem::new("退出", true, None);
    tray_menu.append_items(&[
        &autostart_i,
        &cleanup_i,
        &PredefinedMenuItem::separator(),
        &quit_i,
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::about(
            Some("关于nuna"),
            Some(AboutMetadata {
                name: Some("nuna小程序".to_string()),
                // 与 Cargo.toml 的 version 自动同步，发版只改一处
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
                comments: Some(VERSION_NOTES.to_string()),
                authors: Some(vec!["Nuna".to_string()]),
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
                        .expect("托盘图标创建失败（图标已内嵌且事件循环已启动）"),
                );
            }

            Event::UserEvent(UserEvent::TrayIconEvent(event)) => {
                log::debug!("{event:?}");
            }

            Event::UserEvent(UserEvent::MenuEvent(event)) => {
                log::debug!("{event:?}");
                if event.id == quit_i.id() {
                    tray_icon.take();
                    *control_flow = ControlFlow::Exit;
                } else if event.id == autostart_i.id() {
                    // 菜单已先行翻转勾选态，这里按新状态写注册表；失败则回滚勾选
                    let checked = autostart_i.is_checked();
                    let result = if checked {
                        crate::autostart::enable()
                    } else {
                        crate::autostart::disable()
                    };
                    match result {
                        Ok(()) => log::info!("开机启动已{}", if checked { "启用" } else { "禁用" }),
                        Err(e) => {
                            log::error!("切换开机启动失败: {e}");
                            autostart_i.set_checked(!checked);
                        }
                    }
                } else if event.id == cleanup_i.id() {
                    // 流程含确认弹窗 + UAC + 等子进程 + 结果弹窗，放独立线程避免卡住托盘
                    std::thread::spawn(crate::cleanup::cleanup_flow);
                }
            }

            _ => {}
        }
    })
}

/// 加载自定义托盘图标
/// 图标内嵌进二进制，避免依赖编译期绝对路径（CI 上编译的 `CARGO_MANIFEST_DIR`
/// 在用户机器上不存在，曾导致托盘初始化 panic 后整个程序 0xC0000409 闪退）
///
/// nuna.ico 由 `examples/gen_icon.rs` 生成（Windows 托盘风格的简化键帽+⇪ 箭头），
/// 改设计后重跑该 example 即可，无需外部绘图工具。
fn load_icon() -> tray_icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(include_bytes!("../../assets/nuna.ico"))
            .expect("Failed to decode embedded icon")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
