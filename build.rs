//! 内嵌 Windows 应用程序清单。
//! native-windows-gui 的事件绑定依赖 comctl32 v6 的子类化 API
//! （SetWindowSubclass/GetWindowSubclass），旧版 comctl32 (5.82) 没有这些导出——
//! 不声明 v6 依赖时进程加载即报"无法定位程序输入点 GetWindowSubclass"。
//! embed-manifest 纯 Rust 生成 .res 资源对象，无需 rc.exe。
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = embed_manifest::new_manifest("nuna");
        if let Err(e) = embed_manifest::embed_manifest(manifest) {
            panic!("嵌入应用程序清单失败: {e}");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
