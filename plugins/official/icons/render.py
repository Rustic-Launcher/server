"""Renders the official plugins' icons (original artwork, simple geometry) to SVG and 512 px PNG.

    python plugins/official/icons/render.py

Each icon is a rounded square with a two-colour gradient and a white glyph, defined once in a
512x512 space and written both as SVG (the source) and as PNG (what the marketplace takes: a
square image, stored at 512 px). Needs Pillow.
"""

from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 512
SCALE = 4  # supersampling for smooth edges
HERE = Path(__file__).resolve().parent
GLYPH = (255, 247, 240)
SHADE = (255, 247, 240, 70)

# id -> (top colour, bottom colour, shapes)
# Shapes: ("rect", x0, y0, x1, y1, radius, fill) | ("poly", [(x, y)...], fill) |
#         ("circle", cx, cy, r, fill) | ("ring", cx, cy, r, width, fill)
ICONS = {
    "essentials-lite": (
        (240, 140, 74),
        (184, 74, 28),
        [
            # a house: roof, body, door, window
            ("poly", [(256, 100), (428, 250), (84, 250)], GLYPH),
            ("rect", 140, 236, 372, 408, 14, GLYPH),
            ("rect", 226, 304, 286, 408, 10, "bg"),
            ("rect", 312, 270, 352, 310, 6, "bg"),
            ("rect", 330, 136, 362, 210, 4, GLYPH),
        ],
    ),
    "perms": (
        (58, 186, 170),
        (16, 102, 95),
        [
            # a key: bow, shaft, bit
            ("ring", 190, 196, 78, 34, GLYPH),
            ("poly", [(238, 248), (262, 224), (420, 382), (396, 406)], GLYPH),
            ("poly", [(331, 317), (349, 335), (311, 373), (293, 355)], GLYPH),
            ("poly", [(375, 361), (393, 379), (355, 417), (337, 399)], GLYPH),
        ],
    ),
    "chat-format": (
        (150, 108, 240),
        (82, 50, 168),
        [
            # a speech bubble with three dots and a tail
            ("rect", 96, 120, 416, 340, 56, GLYPH),
            ("poly", [(160, 320), (240, 330), (150, 410)], GLYPH),
            ("circle", 186, 230, 22, "bg"),
            ("circle", 256, 230, 22, "bg"),
            ("circle", 326, 230, 22, "bg"),
        ],
    ),
    "menus": (
        (236, 182, 76),
        (166, 111, 18),
        [
            # a 3x3 grid of menu slots, the centre one picked
            *[
                ("rect", 108 + c * 104, 108 + r * 104, 196 + c * 104, 196 + r * 104, 18,
                 GLYPH if (r, c) == (1, 1) else SHADE)
                for r in range(3)
                for c in range(3)
            ],
            ("rect", 130, 130, 174, 174, 10, GLYPH),
            ("rect", 338, 338, 382, 382, 10, GLYPH),
        ],
    ),
    "mini-game": (
        (234, 96, 110),
        (161, 38, 58),
        [
            # a flag on a pole, on a podium
            ("rect", 150, 100, 178, 404, 10, GLYPH),
            ("poly", [(178, 112), (392, 112), (340, 186), (392, 260), (178, 260)], GLYPH),
            ("rect", 104, 392, 408, 424, 14, GLYPH),
            ("circle", 262, 186, 26, "bg"),
        ],
    ),
}

BG_RADIUS = 112


def hexc(c):
    return "#%02x%02x%02x" % c[:3]


def svg(top, bottom, shapes, mid):
    def fill(f):
        if f == "bg":
            return f'fill="{hexc(mid)}"'
        if len(f) == 4:
            return f'fill="{hexc(f)}" fill-opacity="{f[3] / 255:.2f}"'
        return f'fill="{hexc(f)}"'

    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{SIZE}" height="{SIZE}" viewBox="0 0 {SIZE} {SIZE}">',
        '  <defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1">'
        f'<stop offset="0" stop-color="{hexc(top)}"/><stop offset="1" stop-color="{hexc(bottom)}"/>'
        "</linearGradient></defs>",
        f'  <rect width="{SIZE}" height="{SIZE}" rx="{BG_RADIUS}" fill="url(#g)"/>',
    ]
    for s in shapes:
        kind = s[0]
        if kind == "rect":
            _, x0, y0, x1, y1, r, f = s
            out.append(f'  <rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{y1 - y0}" rx="{r}" {fill(f)}/>')
        elif kind == "poly":
            _, pts, f = s
            out.append(f'  <polygon points="{" ".join(f"{x},{y}" for x, y in pts)}" {fill(f)}/>')
        elif kind == "circle":
            _, cx, cy, r, f = s
            out.append(f'  <circle cx="{cx}" cy="{cy}" r="{r}" {fill(f)}/>')
        elif kind == "ring":
            _, cx, cy, r, w, f = s
            out.append(f'  <circle cx="{cx}" cy="{cy}" r="{r}" fill="none" stroke="{hexc(f)}" stroke-width="{w}"/>')
    out.append("</svg>")
    return "\n".join(out) + "\n"


def png(top, bottom, shapes, mid):
    n = SIZE * SCALE
    grad = Image.new("RGBA", (n, n))
    gd = ImageDraw.Draw(grad)
    for y in range(n):
        t = y / (n - 1)
        gd.line([(0, y), (n, y)], fill=tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)) + (255,))
    mask = Image.new("L", (n, n), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, n - 1, n - 1], radius=BG_RADIUS * SCALE, fill=255)
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    img.paste(grad, (0, 0), mask)
    layer = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    k = SCALE

    def col(f, y=None):
        if f == "bg":
            # the gradient's colour at that height, so cut-outs look like holes
            t = 0.5 if y is None else min(max(y / SIZE, 0), 1)
            return tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)) + (255,)
        return f if len(f) == 4 else f + (255,)

    for s in shapes:
        kind = s[0]
        if kind == "rect":
            _, x0, y0, x1, y1, r, f = s
            d.rounded_rectangle([x0 * k, y0 * k, x1 * k, y1 * k], radius=r * k, fill=col(f, (y0 + y1) / 2))
        elif kind == "poly":
            _, pts, f = s
            d.polygon([(x * k, y * k) for x, y in pts], fill=col(f))
        elif kind == "circle":
            _, cx, cy, r, f = s
            d.ellipse([(cx - r) * k, (cy - r) * k, (cx + r) * k, (cy + r) * k], fill=col(f, cy))
        elif kind == "ring":
            _, cx, cy, r, w, f = s
            ro, ri = r + w / 2, r - w / 2
            d.ellipse([(cx - ro) * k, (cy - ro) * k, (cx + ro) * k, (cy + ro) * k], fill=col(f))
            d.ellipse([(cx - ri) * k, (cy - ri) * k, (cx + ri) * k, (cy + ri) * k], fill=(0, 0, 0, 0))
    # Cut-outs ("bg") were drawn opaque in the gradient's colour; draw the layer over the background.
    img = Image.alpha_composite(img, layer)
    return img.resize((SIZE, SIZE), Image.LANCZOS)


def main():
    for pid, (top, bottom, shapes) in ICONS.items():
        mid = tuple(round((a + b) / 2) for a, b in zip(top, bottom))
        (HERE / f"{pid}.svg").write_text(svg(top, bottom, shapes, mid), encoding="utf-8", newline="\n")
        png(top, bottom, shapes, mid).save(HERE / f"{pid}.png", optimize=True)
        print("wrote", pid)


if __name__ == "__main__":
    main()
