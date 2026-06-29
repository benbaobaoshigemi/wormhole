from __future__ import annotations

import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "wormhole"
SIZES = [16, 24, 32, 48, 64, 128, 256, 512, 1024]


def rounded_rect_mask(size: int, radius: int) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle((0, 0, size - 1, size - 1), radius=radius, fill=255)
    return mask


def draw_icon(size: int) -> Image.Image:
    scale = size / 1024
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    mask = rounded_rect_mask(size, int(220 * scale))

    bg = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    pixels = bg.load()
    for y in range(size):
        for x in range(size):
            t = (x * 0.65 + y * 0.35) / max(1, size - 1)
            r = int(18 + 18 * t)
            g = int(38 + 92 * t)
            b = int(62 + 120 * t)
            pixels[x, y] = (r, g, b, 255)
    bg.putalpha(mask)
    img.alpha_composite(bg)

    glow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    cx, cy = size * 0.5, size * 0.5
    ring_box = (
        int(size * 0.20),
        int(size * 0.27),
        int(size * 0.80),
        int(size * 0.73),
    )
    gd.ellipse(ring_box, outline=(112, 231, 255, 155), width=max(2, int(52 * scale)))
    glow = glow.filter(ImageFilter.GaussianBlur(max(1, int(22 * scale))))
    img.alpha_composite(glow)

    draw = ImageDraw.Draw(img)
    draw.ellipse(ring_box, outline=(96, 224, 244, 255), width=max(2, int(38 * scale)))
    inner_box = (
        int(size * 0.285),
        int(size * 0.345),
        int(size * 0.715),
        int(size * 0.655),
    )
    draw.ellipse(inner_box, outline=(177, 255, 193, 235), width=max(1, int(18 * scale)))

    left = (int(size * 0.31), int(size * 0.50))
    right = (int(size * 0.69), int(size * 0.50))
    for x, y, color in [
        (left[0], left[1], (229, 248, 255, 255)),
        (right[0], right[1], (185, 255, 207, 255)),
    ]:
        radius = max(3, int(70 * scale))
        draw.ellipse(
            (x - radius, y - radius, x + radius, y + radius),
            fill=color,
            outline=(255, 255, 255, 230),
            width=max(1, int(10 * scale)),
        )

    for angle in (-28, 28):
        length = size * 0.28
        rad = math.radians(angle)
        x1 = cx - math.cos(rad) * length / 2
        y1 = cy - math.sin(rad) * length / 2
        x2 = cx + math.cos(rad) * length / 2
        y2 = cy + math.sin(rad) * length / 2
        draw.line(
            (x1, y1, x2, y2),
            fill=(255, 255, 255, 210),
            width=max(1, int(16 * scale)),
        )

    return img


def draw_template_tray_icon(size: int) -> Image.Image:
    scale = size / 32
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    box = (
        int(size * 0.15),
        int(size * 0.25),
        int(size * 0.85),
        int(size * 0.75),
    )
    draw.ellipse(box, outline=(255, 255, 255, 255), width=max(2, int(3 * scale)))
    left = (int(size * 0.33), int(size * 0.50))
    right = (int(size * 0.67), int(size * 0.50))
    radius = max(2, int(4 * scale))
    draw.line((left[0], left[1], right[0], right[1]), fill=(255, 255, 255, 255), width=max(2, int(2 * scale)))
    draw.ellipse(
        (left[0] - radius, left[1] - radius, left[0] + radius, left[1] + radius),
        fill=(255, 255, 255, 255),
    )
    draw.ellipse(
        (right[0] - radius, right[1] - radius, right[0] + radius, right[1] + radius),
        fill=(255, 255, 255, 255),
    )
    return img


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    images = []
    for size in SIZES:
        icon = draw_icon(size)
        icon.save(OUT / f"wormhole-{size}.png")
        images.append(icon)
    images[-1].save(OUT / "wormhole.png")
    images[2].save(OUT / "wormhole-tray.png")
    draw_template_tray_icon(32).save(OUT / "wormhole-tray-template.png")
    images[2].save(OUT / "wormhole.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    images[-1].save(OUT / "Wormhole.icns", sizes=[(16, 16), (32, 32), (128, 128), (256, 256), (512, 512), (1024, 1024)])


if __name__ == "__main__":
    main()
