//! 图标资产生成工具（任务 8）：assets/logo.svg 是唯一设计真相源，
//! 本例程用 resvg 渲染出各尺寸 PNG，交由 tools/package_icon_assets.py
//! 派生 icns / ico / rgba（保证安装后的应用图标与 SVG 完全一致）。
//!
//! 用法：cargo run --example gen_icon_assets

use std::path::Path;

fn main() {
    let svg = include_bytes!("../assets/logo.svg");
    for size in [64u32, 128, 256, 512, 1024] {
        let png = render(svg, size);
        let out = Path::new("assets/render").join(format!("icon_{size}.png"));
        std::fs::create_dir_all("assets/render").unwrap();
        std::fs::write(&out, png).unwrap();
        println!("rendered {out:?}");
    }
}

fn render(svg: &[u8], size: u32) -> Vec<u8> {
    let opt = resvg::usvg::Options {
        default_size: resvg::usvg::Size::from_wh(size as f32, size as f32).unwrap(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_data(svg, &opt).expect("解析 logo.svg");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("创建画布");
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(
            size as f32 / tree.size().width(),
            size as f32 / tree.size().height(),
        ),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().expect("编码 PNG")
}
