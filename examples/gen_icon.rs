//! 托盘图标生成器（一次性工具）：Windows 系统托盘风格的简化 nuna 图标。
//! 设计：圆角键帽（Windows 蓝填充）+ 白色 ⇪ 风格箭头（CapsLock 语义），
//! 填充式双色保证 16px 明暗任务栏都可读。SDF 渲染，边缘 1px 平滑抗锯齿。
//!
//! 用法：cargo run --release --example `gen_icon`
//! 产出：assets/nuna.ico（256px 单帧，tray-icon 经 image crate 解码取该帧）
//!       以及 `assets/icon_preview.png（预览用`）
use image::{ImageFormat, Rgba, RgbaImage};

const SIZE: u32 = 256;
/// Windows 主题蓝 #0078D4 → RGB 分量
const KEYCAP_BLUE: [f64; 3] = [0.0, 120.0, 212.0];
const ARROW_WHITE: [f64; 3] = [255.0; 3];

/// 点到圆角矩形的带符号距离（负=内部）
fn sd_round_rect(px: f64, py: f64, cx: f64, cy: f64, hw: f64, hh: f64, r: f64) -> f64 {
    let dx = (px - cx).abs() - hw + r;
    let dy = (py - cy).abs() - hh + r;
    let (ox, oy) = (dx.max(0.0), dy.max(0.0));
    (ox * ox + oy * oy).sqrt() + dx.max(dy).min(0.0) - r
}

/// 点到三角形的带符号距离（Inigo Quilez 的近似公式，够生成图标用）
fn sd_triangle(px: f64, py: f64, p0: (f64, f64), p1: (f64, f64), p2: (f64, f64)) -> f64 {
    let pts = [p0, p1, p2];
    let mut d = f64::INFINITY;
    let mut s_pos = 0.0f64;
    let mut s_neg = 0.0f64;
    for i in 0..3 {
        let a = pts[i];
        let b = pts[(i + 1) % 3];
        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
        let (wx, wy) = (px - a.0, py - a.1);
        let cross = wx * ey - wy * ex;
        if cross > 0.0 {
            s_pos += 1.0;
        } else if cross < 0.0 {
            s_neg += 1.0;
        }
        let t = (wx * ex + wy * ey) / (ex * ex + ey * ey);
        let (cx, cy) = (px - (a.0 + t * ex), py - (a.1 + t * ey));
        d = d.min((cx * cx + cy * cy).sqrt());
    }
    // 内点判定与绕序无关：三条边的叉积全部同号（或为零）即在三角形内
    if s_pos == 0.0 || s_neg == 0.0 { -d } else { d }
}

/// 距离 → 覆盖率（约 1px 过渡带）
fn coverage(d: f64) -> f64 {
    (0.5 - d).clamp(0.0, 1.0)
}

fn main() {
    let mut img = RgbaImage::new(SIZE, SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);

            // ---- 键帽：宽体圆角矩形（CapsLock 是 1.75u 宽键）----
            let d_cap = sd_round_rect(px, py, 128.0, 136.0, 108.0, 76.0, 30.0);
            let cap = coverage(d_cap);

            // ---- 白色 ⇪ 箭头：实心三角 + 短茎 + 底部横杠（CapsLock 图例样式）----
            // 尺寸给足：16px 缩到任务栏时三角仍有 ~5px 宽的实心面
            let d_tri = sd_triangle(px, py, (128.0, 68.0), (84.0, 122.0), (172.0, 122.0));
            let d_stem = sd_round_rect(px, py, 128.0, 138.0, 14.0, 22.0, 7.0);
            let d_bar = sd_round_rect(px, py, 128.0, 176.0, 38.0, 6.0, 6.0);
            let arrow = coverage(d_tri.min(d_stem).min(d_bar));

            let mut out = [0u8; 4];
            if cap > 0.0 {
                for i in 0..3 {
                    // 箭头与蓝底做普通 alpha 混合，外缘用键帽覆盖率收边。
                    // clamp 到 0..=255 后 as u8 不会截断/变号
                    #[expect(
                        clippy::as_conversions,
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "已 clamp 到 0..=255，f64→u8 无损"
                    )]
                    let byte =
                        (KEYCAP_BLUE[i] * (1.0 - arrow) + ARROW_WHITE[i] * arrow).clamp(0.0, 255.0).round() as u8;
                    out[i] = byte;
                }
                #[expect(
                    clippy::as_conversions,
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "cap∈[0,1]，乘 255 后 clamp 到 0..=255，f64→u8 无损"
                )]
                let alpha = (cap * 255.0).clamp(0.0, 255.0).round() as u8;
                out[3] = alpha;
            }
            img.put_pixel(x, y, Rgba(out));
        }
    }

    img.save_with_format("assets/icon_preview.png", ImageFormat::Png)
        .expect("写入预览 PNG 失败");
    img.save_with_format("assets/nuna.ico", ImageFormat::Ico)
        .expect("写入 ICO 失败");
    println!("已生成 assets/nuna.ico 与 assets/icon_preview.png");
}
