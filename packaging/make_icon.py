#!/usr/bin/env python3
"""Draw the app icon (a black Go stone on a wood board) as PNGs using only the standard library."""
import math, struct, sys, zlib

def png(width, height, pixel):
    raw = bytearray()
    for y in range(height):
        raw.append(0)
        for x in range(width):
            raw.extend(pixel(x, y))
    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xffffffff)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b""))

def blend(a, b, t):
    return tuple(int(a[i] * (1 - t) + b[i] * t) for i in range(3))

def icon(size):
    wood, wood_dark, line = (222, 184, 122), (196, 152, 92), (90, 60, 30)
    stone, stone_hi = (26, 26, 30), (110, 110, 120)
    pad = size * 0.09            # macOS icons leave a margin
    radius = size * 0.20         # rounded-rect corner
    cell = (size - 2 * pad) / 6  # 6 cells of grid
    scx, scy, sr = pad + cell * 3.5, pad + cell * 2.5, cell * 0.46 * 1.6
    def pixel(x, y):
        px, py = x + 0.5, y + 0.5
        # rounded rectangle mask
        qx = max(abs(px - size / 2) - (size / 2 - pad - radius), 0)
        qy = max(abs(py - size / 2) - (size / 2 - pad - radius), 0)
        d = math.hypot(qx, qy) - radius
        if d > 0.75:
            return (0, 0, 0, 0)
        alpha = int(255 * min(1.0, 0.75 - d)) if d > -0.25 else 255
        # wood with soft vertical grain
        grain = 0.5 + 0.5 * math.sin(px / size * 40 + math.sin(py / size * 6) * 2)
        col = blend(wood, wood_dark, grain * 0.35)
        # grid lines
        for i in range(1, 6):
            g = pad + cell * i
            if abs(px - g) < size * 0.008 or abs(py - g) < size * 0.008:
                col = line
        # star point
        if math.hypot(px - (pad + cell * 2), py - (pad + cell * 4)) < size * 0.018:
            col = line
        # stone with shadow and highlight
        sd = math.hypot(px - scx - size * 0.012, py - scy + size * 0.012)
        if sd < sr * 1.06:
            col = blend(col, (60, 40, 20), 0.5 * max(0, 1 - (sd - sr * 0.9) / (sr * 0.16)) if sd > sr * 0.9 else 0.5)
        dist = math.hypot(px - scx, py - scy)
        if dist < sr + 0.75:
            t = min(1.0, sr + 0.75 - dist)
            hx, hy = scx - sr * 0.35, scy - sr * 0.4
            h = max(0.0, 1 - math.hypot(px - hx, py - hy) / (sr * 1.1))
            c = blend(stone, stone_hi, h ** 2)
            col = blend(col, c, t)
        return (*col, alpha)
    return png(size, size, pixel)

if __name__ == "__main__":
    out_dir = sys.argv[1]
    for s in (16, 32, 64, 128, 256, 512, 1024):
        with open(f"{out_dir}/icon_{s}.png", "wb") as f:
            f.write(icon(s))
