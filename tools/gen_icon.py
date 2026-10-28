#!/usr/bin/env python3
"""黄瓜图标生成器：纯 Python（zlib + struct 手写 PNG），无第三方依赖。

产出（相对 repo 根）：
  assets/icon_512.png   主图（README/发布页展示）
  assets/icon_128.rgba  窗口图标原始像素（include_bytes! 进二进制）
  assets/icon.ico       Windows exe 资源图标（PNG-in-ICO，256px）
  assets/AppIcon.icns   macOS .app 图标（内嵌 256/512/1024 PNG）

设计：绿色渐变圆角底板 + 斜置黄瓜（胶囊体 + 高光 + 疣点 + 花端）。
造型参数在此处唯一维护，重跑即可复现全部资产。
"""

import math
import os
import struct
import zlib

OUT = os.path.join(os.path.dirname(__file__), "..", "assets")

# ---------- 配色 ----------
BG_TOP = (0x35, 0xB5, 0x5A)   # 底板渐变顶部
BG_BOT = (0x1C, 0x7A, 0x38)   # 底板渐变底部
MELON = (0x0F, 0x4D, 0x22)    # 瓜体主色
MELON_HI = (0x6F, 0xD9, 0x8A) # 瓜体高光
MELON_LO = (0x0A, 0x38, 0x18) # 瓜体暗侧
BLOSSOM = (0xE9, 0xF6, 0xC8)  # 花端
WART = (0xDC, 0xF2, 0xDF)     # 疣点


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def smooth(edge0, edge1, x):
    t = min(1.0, max(0.0, (x - edge0) / (edge1 - edge0)))
    return t * t * (3 - 2 * t)


def render(size):
    """按尺寸渲染，返回 rgba 字节串。坐标归一化，y 向下。"""
    px = bytearray(size * size * 4)
    s = float(size)
    corner = 0.225 * s  # 圆角半径

    # 黄瓜中心线：从花端(左上)到茎端(右下)
    (x0, y0), (x1, y1) = (0.375 * s, 0.275 * s), (0.645 * s, 0.745 * s)
    ax, ay = x1 - x0, y1 - y0
    axis_len = math.hypot(ax, ay)
    ux, uy = ax / axis_len, ay / axis_len          # 轴向单位向量
    vx, vy = -uy, ux                                # 垂直向量

    def half_width(t):
        # 端部收窄的胶囊轮廓
        tip = min(1.0, abs(t - 0.5) * 2.0)          # 0 中心 → 1 端点
        return (0.068 + 0.045 * math.sin(math.pi * (1 - tip) ** 0.85)) * s

    warts = [  # (t 沿轴, s 偏移比例, 半径)
        (0.30, -0.25, 0.016), (0.46, 0.30, 0.015), (0.58, -0.32, 0.014),
        (0.70, 0.20, 0.013), (0.38, 0.42, 0.011), (0.82, -0.15, 0.012),
    ]

    for yy in range(size):
        for xx in range(size):
            fx, fy = xx + 0.5, yy + 0.5
            # --- 圆角矩形底板（sdRoundBox）---
            qx, qy = abs(fx - s / 2) - (s / 2 - corner), abs(fy - s / 2) - (s / 2 - corner)
            sd = min(max(qx, qy), 0.0) + math.hypot(max(qx, 0.0), max(qy, 0.0)) - corner
            bg_a = smooth(0.75, -0.75, sd)          # 1 内 0 外（1.5px 抗锯齿）
            if bg_a <= 0:
                continue
            base = lerp(BG_TOP, BG_BOT, fy / s)
            # --- 黄瓜体 ---
            dx, dy = fx - x0, fy - y0
            t = (dx * ux + dy * uy) / axis_len
            d = dx * vx + dy * vy                    # 垂直偏移（有符号）
            hw = half_width(t)
            body = smooth(hw + 0.75, hw - 0.75, abs(d)) * smooth(0.02, -0.75, t) * smooth(0.98, 1.75, t)
            if body > 0:
                # 基色：暗侧到亮侧的横向渐变
                k = 0.5 - 0.5 * (d / hw)             # -1..1 → 1..0
                col = lerp(MELON_LO, MELON, min(1.0, max(0.0, k)))
                # 高光带：暗侧上方细条
                hi = smooth(hw * 0.28, hw * 0.10, abs(d + hw * 0.34)) * smooth(0.12, 0.30, t) * smooth(0.92, 0.72, t)
                col = lerp(col, MELON_HI, hi * 0.85)
                # 花端：t 小端一小段浅黄
                bl = smooth(0.10, 0.045, t) * smooth(hw, hw * 0.55, abs(d))
                col = lerp(col, BLOSSOM, bl)
                # 疣点
                for wt, ws, wr in warts:
                    wx, wy = x0 + ux * t * 0 + (x0 + ax * wt) + vx * (ws * hw), 0
                    cx = x0 + ax * wt + vx * (ws * hw)
                    cy = y0 + ay * wt + vy * (ws * hw)
                    wd = math.hypot(fx - cx, fy - cy) / s
                    col = lerp(col, WART, smooth(wr, wr - 0.008, wd) * 0.9)
                # 瓜体在底板上的投影感：底部轻微压暗（沿 d 正方向）
                col = lerp(col, MELON_LO, smooth(-hw, hw, d) * 0.12)
                base = lerp(base, col, body)
            r, g, b = base
            i = (yy * size + xx) * 4
            px[i] = r
            px[i + 1] = g
            px[i + 2] = b
            px[i + 3] = int(255 * bg_a)
    return bytes(px)


# ---------- PNG / ICO / ICNS 编码 ----------

def png_chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


def encode_png(size, rgba):
    raw = b"".join(b"\x00" + rgba[y * size * 4:(y + 1) * size * 4] for y in range(size))
    return (b"\x89PNG\r\n\x1a\n"
            + png_chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
            + png_chunk(b"IDAT", zlib.compress(raw, 9))
            + png_chunk(b"IEND", b""))


def encode_ico(png_bytes, size):
    # PNG-in-ICO（Vista+ 支持）
    hdr = struct.pack("<HHH", 0, 1, 1)
    entry = struct.pack("<BBBBHHII", 0, 0, 0, 0, 1, 32, len(png_bytes), 6 + 16)
    return hdr + entry + png_bytes


def encode_icns(pngs):  # [(type_code, png_bytes)]
    body = b"".join(tag + struct.pack(">I", len(png) + 8) + png for tag, png in pngs)
    return b"icns" + struct.pack(">I", len(body) + 8) + body


def main():
    os.makedirs(OUT, exist_ok=True)
    rgba512 = render(512)
    rgba1024 = render(1024)
    rgba128 = render(128)
    open(os.path.join(OUT, "icon_512.png"), "wb").write(encode_png(512, rgba512))
    open(os.path.join(OUT, "icon_128.rgba"), "wb").write(rgba128)
    open(os.path.join(OUT, "icon.ico"), "wb").write(encode_ico(encode_png(256, render(256)), 256))
    open(os.path.join(OUT, "AppIcon.icns"), "wb").write(
        encode_icns([(b"ic08", encode_png(256, render(256))),
                     (b"ic09", encode_png(512, rgba512)),
                     (b"ic10", encode_png(1024, rgba1024))]))
    print("assets:", ", ".join(sorted(os.listdir(OUT))))


if __name__ == "__main__":
    main()
