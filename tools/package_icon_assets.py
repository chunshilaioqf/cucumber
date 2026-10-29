#!/usr/bin/env python3
"""从 resvg 渲染的 SVG PNG（assets/render/icon_*.png，由 examples/gen_icon_assets.rs 产出）
派生发布资产（任务 8）：icns / ico / 窗口 RGBA / 托盘 RGBA —— 保证与 SVG 图形完全一致。"""
import struct, zlib, os

ROOT = os.path.join(os.path.dirname(__file__), "..", "assets")

def png_chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

def read_rgba(path):
    from PIL import Image
    with Image.open(path) as im:
        return im.size[0], im.tobytes()
    # 无 PIL 时不可达：脚本在安装了 Pillow 的环境运行

def encode_ico(png_bytes):
    hdr = struct.pack("<HHH", 0, 1, 1)
    entry = struct.pack("<BBBBHHII", 0, 0, 0, 0, 1, 32, len(png_bytes), 6 + 16)
    return hdr + entry + png_bytes

def encode_icns(pngs):
    body = b"".join(tag + struct.pack(">I", len(png) + 8) + png for tag, png in pngs)
    return b"icns" + struct.pack(">I", len(body) + 8) + body

def main():
    png = lambda n: open(os.path.join(ROOT, "render", f"icon_{n}.png"), "rb").read()
    open(os.path.join(ROOT, "icon.ico"), "wb").write(encode_ico(png(256)))
    open(os.path.join(ROOT, "AppIcon.icns"), "wb").write(
        encode_icns([(b"ic08", png(256)), (b"ic09", png(512)), (b"ic10", png(1024))]))
    for size, name in [(128, "icon_128.rgba"), (64, "icon_64.rgba")]:
        w, rgba = read_rgba(os.path.join(ROOT, "render", f"icon_{size}.png"))
        assert w == size and len(rgba) == size * size * 4
        open(os.path.join(ROOT, name), "wb").write(rgba)
    # 主展示图与 DMG 背景源同步
    open(os.path.join(ROOT, "icon_512.png"), "wb").write(png(512))
    print("icon.ico / AppIcon.icns / icon_128.rgba / icon_64.rgba 已从 SVG 渲染结果派生")

if __name__ == "__main__":
    main()
