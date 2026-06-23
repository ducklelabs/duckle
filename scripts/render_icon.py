"""Render the Duckle "d" logo to a high-res PNG for `tauri icon`.

Reproduces the brand mark (docs/assets/duckle-logo-light.svg): a warm rounded
tile with the two-tone lowercase "d" - a peach bowl ring, an orange ascender
stem, and the deeper overlap where they cross. Drawn at 4x and downscaled with
LANCZOS for smooth edges. Run from the repo root, then regenerate the icon set:

    python scripts/render_icon.py
    cargo tauri icon apps/desktop/icons/icon-source.png   # from apps/desktop
"""

from PIL import Image, ImageDraw

S = 1024            # output size
SS = 4              # supersample factor
W = S * SS          # working size

# Brand palette (sampled from the new logo art).
CREAM = (0xFB, 0xF3, 0xE8, 255)   # warm tile background
PEACH = (0xF6, 0xBA, 0x78, 255)   # bowl ring
ORANGE = (0xEA, 0x7E, 0x42, 255)  # ascender stem
DEEP = (0xD9, 0x74, 0x2F, 255)    # overlap wedge

# Map the 64-unit mark viewBox (glyph centred on 32,32) into the canvas so the
# glyph fills ~56% of the tile height.
SCALE = 0.56 * W / 44.0


def mx(x):
    return W / 2 + (x - 32) * SCALE


def my(y):
    return W / 2 + (y - 32) * SCALE


def msz(v):
    return v * SCALE


def main():
    img = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # Warm rounded tile (transparent corners -> rounded app icon).
    d.rounded_rectangle([0, 0, W - 1, W - 1], radius=int(0.20 * W), fill=CREAM)

    # Bowl outer disc (peach), then punch the counter back to the tile colour.
    cx, cy, r = mx(32), my(38), msz(16)
    d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=PEACH)
    ccx, ccy, cr = mx(31), my(38), msz(7)
    d.ellipse([ccx - cr, ccy - cr, ccx + cr, ccy + cr], fill=CREAM)

    # Ascender stem (orange rounded bar).
    stem = [mx(40), my(10), mx(48), my(54)]
    srad = msz(4)
    d.rounded_rectangle(stem, radius=srad, fill=ORANGE)

    # Overlap: the part of the stem inside the bowl disc, in the deeper colour.
    overlay = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    ImageDraw.Draw(overlay).rounded_rectangle(stem, radius=srad, fill=DEEP)
    circle = Image.new("L", (W, W), 0)
    ImageDraw.Draw(circle).ellipse([cx - r, cy - r, cx + r, cy + r], fill=255)
    kept = Image.composite(overlay.getchannel("A"), Image.new("L", (W, W), 0), circle)
    overlay.putalpha(kept)
    img = Image.alpha_composite(img, overlay)

    img = img.resize((S, S), Image.LANCZOS)
    out = "apps/desktop/icons/icon-source.png"
    img.save(out)
    print("wrote", out, img.size)


if __name__ == "__main__":
    main()
