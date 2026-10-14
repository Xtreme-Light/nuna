//! 按键映射配置窗口（类似 SharpKeys 的配置方式）：
//! 左侧"原始键位"、右侧"映射为"，各自通过点击"采集"后直接按组合键录入；
//! 列表展示全部映射，支持删除、恢复默认；"保存并应用"写入 config.json
//! 并立即替换拦截线程正在使用的映射表（无需重启）。
//!
//! 窗口运行在独立线程上（原生 Win32 控件，各自消息循环，与托盘的 tao 循环互不干扰）。
//! 采集期间拦截线程会把所有按键吞掉只做记录，因此按键不会真的生效
//! （例如采集 Caps 时不会切换大小写锁定）。

use crate::mapping::{self, CaptureMsg, KeyMapping};
use crate::AppShared;
use native_windows_gui as nwg;
use std::cell::RefCell;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 打开配置窗口；若已打开则忽略（单实例）
pub fn open(shared: Arc<AppShared>) {
    if shared.ui_open.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        log::info!("打开按键配置窗口");
        if let Err(e) = run_ui(shared.clone()) {
            log::error!("配置窗口出错: {e:#}");
            crate::fatal_dialog("nuna 配置窗口出错", &format!("{e:#}"));
        }
        // 兜底：关窗后采集模式必须关闭，否则键盘会被一直吞掉
        shared.capture_on.store(false, Ordering::SeqCst);
        shared.ui_open.store(false, Ordering::SeqCst);
        log::info!("按键配置窗口已关闭");
    });
}

/// 正在采集哪个槽位
#[derive(Clone, Copy, PartialEq)]
enum Slot {
    Src,
    Dst,
}

/// 采集到的一个组合
#[derive(Clone, Copy)]
struct Combo {
    mods: u16,
    code: u16,
    ext: bool,
}

impl Combo {
    fn display(&self) -> String {
        mapping::combo_display(self.mods, self.code, self.ext)
    }
}

struct UiState {
    rows: Vec<KeyMapping>,
    src: Option<Combo>,
    dst: Option<Combo>,
    capturing: Option<Slot>,
}

fn run_ui(shared: Arc<AppShared>) -> Result<(), nwg::NwgError> {
    nwg::init()?;

    let mut window = Default::default();
    nwg::Window::builder()
        .size((660, 520))
        .position((240, 180))
        .title("nuna 按键映射配置")
        .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
        .build(&mut window)?;

    let mut lbl_src = Default::default();
    nwg::Label::builder()
        .text("原始键位:")
        .size((72, 22))
        .position((12, 16))
        .parent(&window)
        .flags(nwg::LabelFlags::VISIBLE)
        .build(&mut lbl_src)?;

    let mut src_txt = Default::default();
    nwg::TextInput::builder()
        .text("（未采集）")
        .size((330, 26))
        .position((88, 12))
        .parent(&window)
        .flags(nwg::TextInputFlags::VISIBLE)
        .readonly(true)
        .build(&mut src_txt)?;

    let mut cap_src_btn = Default::default();
    nwg::Button::builder()
        .text("采集")
        .size((90, 28))
        .position((430, 11))
        .parent(&window)
        .flags(nwg::ButtonFlags::VISIBLE)
        .build(&mut cap_src_btn)?;

    let mut lbl_dst = Default::default();
    nwg::Label::builder()
        .text("映射为:")
        .size((72, 22))
        .position((12, 50))
        .parent(&window)
        .flags(nwg::LabelFlags::VISIBLE)
        .build(&mut lbl_dst)?;

    let mut dst_txt = Default::default();
    nwg::TextInput::builder()
        .text("（未采集）")
        .size((330, 26))
        .position((88, 46))
        .parent(&window)
        .flags(nwg::TextInputFlags::VISIBLE)
        .readonly(true)
        .build(&mut dst_txt)?;

    let mut cap_dst_btn = Default::default();
    nwg::Button::builder()
        .text("采集")
        .size((90, 28))
        .position((430, 45))
        .parent(&window)
        .flags(nwg::ButtonFlags::VISIBLE)
        .build(&mut cap_dst_btn)?;

    let mut hint = Default::default();
    nwg::Label::builder()
        .text("点\"采集\"后直接按组合键（如先按住 CapsLock 再按 C；Esc 取消）。保存后立即生效。")
        .size((620, 20))
        .position((12, 84))
        .parent(&window)
        .flags(nwg::LabelFlags::VISIBLE)
        .build(&mut hint)?;

    let mut list = Default::default();
    nwg::ListView::builder()
        .size((620, 290))
        .position((12, 110))
        .parent(&window)
        .flags(nwg::ListViewFlags::VISIBLE | nwg::ListViewFlags::SINGLE_SELECTION)
        .ex_flags(nwg::ListViewExFlags::GRID | nwg::ListViewExFlags::FULL_ROW_SELECT)
        .build(&mut list)?;
    list.set_headers_enabled(true);
    list.insert_column("原始键位");
    list.insert_column("映射为");

    let mut add_btn = Default::default();
    nwg::Button::builder()
        .text("添加映射")
        .size((110, 30))
        .position((12, 412))
        .parent(&window)
        .flags(nwg::ButtonFlags::VISIBLE)
        .build(&mut add_btn)?;

    let mut del_btn = Default::default();
    nwg::Button::builder()
        .text("删除选中")
        .size((110, 30))
        .position((132, 412))
        .parent(&window)
        .flags(nwg::ButtonFlags::VISIBLE)
        .build(&mut del_btn)?;

    let mut reset_btn = Default::default();
    nwg::Button::builder()
        .text("恢复默认")
        .size((110, 30))
        .position((252, 412))
        .parent(&window)
        .flags(nwg::ButtonFlags::VISIBLE)
        .build(&mut reset_btn)?;

    let mut save_btn = Default::default();
    nwg::Button::builder()
        .text("保存并应用")
        .size((130, 30))
        .position((502, 412))
        .parent(&window)
        .flags(nwg::ButtonFlags::VISIBLE)
        .build(&mut save_btn)?;

    let mut status = Default::default();
    nwg::Label::builder()
        .text("")
        .size((620, 24))
        .position((12, 452))
        .parent(&window)
        .flags(nwg::LabelFlags::VISIBLE)
        .build(&mut status)?;

    let mut timer = Default::default();
    nwg::AnimationTimer::builder()
        .parent(&window)
        .interval(std::time::Duration::from_millis(100))
        .build(&mut timer)?;

    // 初始状态 = 拦截线程当前正在使用的映射
    let init_rows = shared.mappings.lock().unwrap().clone();
    refresh_list(&list, &init_rows);

    let state = RefCell::new(UiState {
        rows: init_rows,
        src: None,
        dst: None,
        capturing: None,
    });

    let window_handle = window.handle.clone();
    let handler = nwg::full_bind_event_handler(&window_handle, move |evt, _data, handle| {
        use nwg::Event;
        match evt {
            Event::OnButtonClick => {
                if handle == cap_src_btn {
                    state.borrow_mut().capturing = Some(Slot::Src);
                    shared.capture_on.store(true, Ordering::SeqCst);
                    src_txt.set_text("请按下组合键…（Esc 取消）");
                    status.set_text("");
                } else if handle == cap_dst_btn {
                    state.borrow_mut().capturing = Some(Slot::Dst);
                    shared.capture_on.store(true, Ordering::SeqCst);
                    dst_txt.set_text("请按下组合键…（Esc 取消）");
                    status.set_text("");
                } else if handle == add_btn {
                    let mut st = state.borrow_mut();
                    if let (Some(src), Some(dst)) = (st.src, st.dst) {
                        st.rows.push(KeyMapping {
                            src_mods: src.mods,
                            src_code: src.code,
                            src_ext: src.ext,
                            dst_mods: dst.mods,
                            dst_code: dst.code,
                            dst_ext: dst.ext,
                        });
                        refresh_list(&list, &st.rows);
                        status.set_text("已添加到列表（尚未保存）");
                        st.src = None;
                        st.dst = None;
                        src_txt.set_text("（未采集）");
                        dst_txt.set_text("（未采集）");
                    } else {
                        status.set_text("请先采集原始键位和映射目标");
                    }
                } else if handle == del_btn {
                    let mut st = state.borrow_mut();
                    if let Some(idx) = list.selected_item() {
                        if idx < st.rows.len() {
                            st.rows.remove(idx);
                            refresh_list(&list, &st.rows);
                            status.set_text("已删除（尚未保存）");
                        }
                    } else {
                        status.set_text("请先在列表中选中一行");
                    }
                } else if handle == reset_btn {
                    let mut st = state.borrow_mut();
                    st.rows = mapping::default_mappings();
                    refresh_list(&list, &st.rows);
                    status.set_text("已恢复默认键位（尚未保存）");
                } else if handle == save_btn {
                    let st = state.borrow();
                    let n = st.rows.len();
                    match mapping::save_config(&st.rows) {
                        Ok(()) => {
                            *shared.mappings.lock().unwrap() = st.rows.clone();
                            status.set_text(&format!("已保存并应用，共 {n} 条映射"));
                        }
                        Err(e) => {
                            status.set_text(&format!("保存失败：{e}"));
                        }
                    }
                }
            }
            Event::OnTimerTick => {
                if handle == timer {
                    // 轮询采集结果通道
                    let slot = state.borrow().capturing;
                    if let Some(slot) = slot {
                        if let Ok(msg) = shared.capture_rx.try_recv() {
                            let mut st = state.borrow_mut();
                            st.capturing = None;
                            match msg {
                                CaptureMsg::Combo { mods, code, ext } => {
                                    let c = Combo { mods, code, ext };
                                    match slot {
                                        Slot::Src => {
                                            st.src = Some(c);
                                            src_txt.set_text(&c.display());
                                        }
                                        Slot::Dst => {
                                            st.dst = Some(c);
                                            dst_txt.set_text(&c.display());
                                        }
                                    }
                                    status.set_text("采集完成");
                                }
                                CaptureMsg::Cancelled => {
                                    status.set_text("已取消采集");
                                }
                            }
                        }
                    }
                }
            }
            Event::OnWindowClose => {
                if handle == window {
                    nwg::stop_thread_dispatch();
                }
            }
            _ => {}
        }
    });

    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);
    Ok(())
}

/// 重建列表内容（两列：原始键位 / 映射为）
fn refresh_list(list: &nwg::ListView, rows: &[KeyMapping]) {
    list.clear();
    for m in rows {
        let row = list.len() as i32;
        list.insert_item(m.src_display().as_str());
        list.insert_item(nwg::InsertListViewItem {
            index: Some(row),
            column_index: 1,
            text: Some(m.dst_display()),
            ..Default::default()
        });
    }
}
