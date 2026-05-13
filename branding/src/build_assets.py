"""
TMAIL-191 — fan out the SVG primary marks into the full raster asset library.

Inputs (run build_logo.py first):
  branding/build/svg/logo-primary.svg
  branding/build/svg/logo-tile-blue.svg
  branding/build/svg/logo-mono-{black,white}.svg
  branding/build/wordmark/wordmark-{light,dark,blue}.svg

Outputs (under branding/build):
  app-icons/icon-{16,24,32,48,64,96,128,192,256,384,512,1024}.png
  png/apple-touch-icon.png
  png/icon-192.png  png/icon-512.png
  png/maskable-192.png  png/maskable-512.png
  png/email-signature.png
  png/logo-2048.png
  social/og-card.png       (1200x630, charcoal bg, mark + wordmark)
  social/twitter-card.png  (1200x600)
  ico/favicon.ico          (16, 32, 48 multi-resolution)

After this, run optipng / pngquant on the PNGs to shrink them.
"""
from __future__ import annotations
from pathlib import Path
import io

import cairosvg
from PIL import Image, ImageDraw, ImageFont

ROOT  = Path(__file__).resolve().parents[1] / "build"
SVG   = ROOT / "svg"
WORD  = ROOT / "wordmark"
APPS  = ROOT / "app-icons"
PNG   = ROOT / "png"
SOC   = ROOT / "social"
ICO   = ROOT / "ico"
for d in (APPS, PNG, SOC, ICO):
    d.mkdir(parents=True, exist_ok=True)

# Brand palette mirrored from BRAND.md so the rasterisers can paint surrounds
# without re-reading the spec.
BLUE       = (0x25, 0x63, 0xeb)
CHARCOAL   = (0x0f, 0x17, 0x2a)
WHITE      = (0xff, 0xff, 0xff)
SURFACE    = (0xf8, 0xfa, 0xfc)


def render(svg_path: Path, size: int) -> Image.Image:
    """Rasterise an SVG at `size` x `size` pixels and return a Pillow image."""
    png = cairosvg.svg2png(url=str(svg_path), output_width=size, output_height=size)
    return Image.open(io.BytesIO(png)).convert("RGBA")


def render_size(svg_path: Path, w: int, h: int) -> Image.Image:
    png = cairosvg.svg2png(url=str(svg_path), output_width=w, output_height=h)
    return Image.open(io.BytesIO(png)).convert("RGBA")


def write_png(img: Image.Image, path: Path) -> None:
    img.save(path, format="PNG", optimize=True)
    print(f"  {path.relative_to(ROOT.parent)}  ({img.size[0]}×{img.size[1]})")


# -----------------------------------------------------------------------------
# 1. App icons — 11 sizes, transparent bg, primary mark
# -----------------------------------------------------------------------------
print("App icons (transparent):")
for size in (16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 1024):
    write_png(render(SVG / "logo-primary.svg", size), APPS / f"icon-{size}.png")

# -----------------------------------------------------------------------------
# 2. PWA icons — square brand-blue tile (matches manifest theme_color)
# -----------------------------------------------------------------------------
print("PWA icons (blue tile):")
write_png(render(SVG / "logo-tile-blue.svg", 192), PNG / "icon-192.png")
write_png(render(SVG / "logo-tile-blue.svg", 512), PNG / "icon-512.png")

# Maskable variants need a full-bleed brand-coloured background (the system
# crops to a circle/rounded-square so corners may disappear). The actual mark
# lives in the central ~80% safe area. We render the *bare* envelope (white
# stroke, transparent bg via logo-dark.svg) onto a flat brand-blue canvas so
# we don't end up with two stacked rounded rectangles.
print("PWA icons (maskable, 80% safe area):")
def maskable(target: int) -> Image.Image:
    bg = Image.new("RGBA", (target, target), BLUE + (255,))
    inner_size = int(target * 0.78)  # 11% padding on each side ≈ 78% safe area
    inner = render(SVG / "logo-dark.svg", inner_size)
    pad = (target - inner_size) // 2
    bg.paste(inner, (pad, pad), inner)
    return bg

write_png(maskable(192), PNG / "maskable-192.png")
write_png(maskable(512), PNG / "maskable-512.png")

# -----------------------------------------------------------------------------
# 3. Apple touch icon — 180x180 brand tile
# -----------------------------------------------------------------------------
print("Apple touch icon:")
write_png(render(SVG / "logo-tile-blue.svg", 180), PNG / "apple-touch-icon.png")

# -----------------------------------------------------------------------------
# 4. Email signature — 200x200, transparent so it sits on any sig theme
# -----------------------------------------------------------------------------
print("Email signature:")
write_png(render(SVG / "logo-primary.svg", 200), PNG / "email-signature.png")
write_png(render(SVG / "logo-primary.svg", 96),  PNG / "email-signature-96.png")

# -----------------------------------------------------------------------------
# 5. Print master — 2048 transparent
# -----------------------------------------------------------------------------
print("Print master:")
write_png(render(SVG / "logo-primary.svg", 2048), PNG / "logo-2048.png")

# -----------------------------------------------------------------------------
# 6. Social cards (Open Graph + Twitter) — charcoal bg, mark + wordmark + tag
# -----------------------------------------------------------------------------
def social_card(width: int, height: int, out: Path, tagline: str) -> None:
    canvas = Image.new("RGBA", (width, height), CHARCOAL + (255,))

    # Subtle radial accent in the top-left corner — paints the brand-blue glow
    # that the landing hero uses, giving the card a matching feel.
    accent = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    draw_acc = ImageDraw.Draw(accent)
    cx, cy = int(width * 0.12), int(height * 0.18)
    radius = int(min(width, height) * 0.55)
    for i in range(radius, 0, -8):
        alpha = int(38 * (i / radius) ** 2)
        bbox = (cx - i, cy - i, cx + i, cy + i)
        draw_acc.ellipse(bbox, fill=BLUE + (max(0, 60 - alpha),))
    canvas.alpha_composite(accent)

    # Mark — square ~36% of card height, top-left
    mark_size = int(height * 0.36)
    mark = render(SVG / "logo-tile-blue.svg", mark_size)
    canvas.paste(mark, (int(width * 0.06), int(height * 0.18)), mark)

    # Wordmark + tagline next to the mark
    draw = ImageDraw.Draw(canvas)
    title_x = int(width * 0.06) + mark_size + int(width * 0.04)
    title_y = int(height * 0.30)

    # System fonts to avoid bundling — fall back to Pillow default if missing.
    def font(size: int, weight: str = "Bold") -> ImageFont.FreeTypeFont:
        candidates = [
            f"/usr/share/fonts/truetype/dejavu/DejaVuSans-{weight}.ttf",
            f"/usr/share/fonts/truetype/liberation/LiberationSans-{weight}.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        ]
        for path in candidates:
            if Path(path).exists():
                return ImageFont.truetype(path, size)
        return ImageFont.load_default()

    draw.text((title_x, title_y), "TASMail",
              font=font(int(height * 0.16), "Bold"), fill=WHITE + (255,))
    draw.text((title_x, title_y + int(height * 0.18)), tagline,
              font=font(int(height * 0.05), ""), fill=(0xcb, 0xd5, 0xe1, 255))

    # Footer URL right-aligned
    url = "mail.techatscale.io"
    f_url = font(int(height * 0.04), "")
    bbox = draw.textbbox((0, 0), url, font=f_url)
    url_w = bbox[2] - bbox[0]
    draw.text((width - url_w - int(width * 0.04),
               height - int(height * 0.07)),
              url, font=f_url, fill=(0x94, 0xa3, 0xb8, 255))

    canvas.convert("RGB").save(out, format="PNG", optimize=True)
    print(f"  {out.relative_to(ROOT.parent)}  ({width}×{height})")

print("Social cards:")
social_card(1200, 630, SOC / "og-card.png",      "Webmail for any IMAP/SMTP server")
social_card(1200, 600, SOC / "twitter-card.png", "Webmail for any IMAP/SMTP server")

# -----------------------------------------------------------------------------
# 7. Favicon multi-resolution .ico
# -----------------------------------------------------------------------------
print("Favicon ICO:")
fav_imgs = [render(SVG / "logo-primary.svg", s) for s in (16, 32, 48)]
fav_imgs[0].save(
    ICO / "favicon.ico",
    sizes=[(16, 16), (32, 32), (48, 48)],
    append_images=fav_imgs[1:],
)
print(f"  {(ICO / 'favicon.ico').relative_to(ROOT.parent)}  (16/32/48)")

# -----------------------------------------------------------------------------
# 8. Wordmark PNGs (light + dark) for places that can't use SVG (e.g., emails)
# -----------------------------------------------------------------------------
print("Wordmark PNGs:")
write_png(render_size(WORD / "wordmark-light.svg", 480, 112), WORD / "wordmark-light.png")
write_png(render_size(WORD / "wordmark-dark.svg",  480, 112), WORD / "wordmark-dark.png")
write_png(render_size(WORD / "wordmark-blue.svg",  480, 112), WORD / "wordmark-blue.png")

print("\nDone. Run optipng/pngquant on branding/build/**/*.png to shrink further.")
