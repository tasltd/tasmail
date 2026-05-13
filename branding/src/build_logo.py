"""
TMAIL-190 — generator for the TASMail primary mark + variants.

Hand-writes SVG so every coordinate is intentional. Output goes to
branding/build/svg/. Run from the project root:

    python3 branding/src/build_logo.py

The geometry follows BRAND.md: 24-unit grid, envelope occupies (4..20, 6..18),
flap diagonals meet at (12, 14), inner wordmark `t@s` baseline at y=14.5.
"""
from pathlib import Path
from textwrap import dedent

ROOT = Path(__file__).resolve().parents[1] / "build" / "svg"
ROOT.mkdir(parents=True, exist_ok=True)

# ---- palette -----------------------------------------------------------------
BLUE       = "#2563eb"
BLUE_DARK  = "#1d4ed8"
PURPLE     = "#7c3aed"
TEAL       = "#2dd4bf"
CHARCOAL   = "#0f172a"
CHARCOAL_2 = "#334155"
SURFACE    = "#f8fafc"
WHITE      = "#ffffff"


def _envelope_paths(stroke: str, accent_at: str, *, mask_bg: str | None = None) -> str:
    """
    Returns the inner SVG body for the primary mark.

    Composition:
      * envelope outline (rounded rectangle)
      * top flap — single inverted V from the top corners meeting at midpoint
      * `t@s` wordmark sitting in the open envelope below the flap, with an
        optional `mask_bg`-coloured rounded rectangle behind it to keep the
        glyphs legible if they ever overlap the diagonals on smaller renders.

    All coordinates are in a 24-unit grid scaled by the SVG viewBox.
    """
    # The flap is a single ^ (no bottom crossing) so the inner wordmark sits in
    # a fully clean lower half of the envelope. This is the cleanest reading at
    # 16px and keeps the t@s visually anchored.
    mask_layer = ""
    if mask_bg is not None:
        # Subtle backdrop with same fill as the envelope's surrounding area, so
        # any future use that reintroduces crossing diagonals doesn't damage
        # the wordmark legibility.
        mask_layer = (
            f'<rect x="6.4" y="13.0" width="11.2" height="3.4" rx="0.6" '
            f'fill="{mask_bg}"/>'
        )
    return dedent(f"""\
        <!-- envelope outline -->
        <rect x="3.5" y="6"  width="17" height="12"
              rx="1.4" ry="1.4"
              fill="none" stroke="{stroke}" stroke-width="1.6"
              stroke-linejoin="round"/>
        <!-- top flap: single inverted V meeting at the midpoint -->
        <path d="M 3.5 6 L 12 13.2 L 20.5 6"
              fill="none" stroke="{stroke}" stroke-width="1.6"
              stroke-linejoin="round" stroke-linecap="round"/>
        {mask_layer}
        <!-- inner wordmark t@s — baseline y=16.6 (lower half of envelope) -->
        <g font-family="'Inter','SF Pro Display','Segoe UI',system-ui,sans-serif"
           font-weight="700"
           text-anchor="middle">
          <text x="8.6"  y="16.7" font-size="4.6" letter-spacing="-0.12" fill="{stroke}">t</text>
          <text x="12"   y="16.95" font-size="5.2" letter-spacing="-0.12" fill="{accent_at}">@</text>
          <text x="15.4" y="16.7" font-size="4.6" letter-spacing="-0.12" fill="{stroke}">s</text>
        </g>
    """).strip()


def primary_svg(stroke: str, accent_at: str, *, bg: str | None = None,
                bg_corner_radius: float | None = None) -> str:
    """
    Wrap _envelope_paths in a 24×24 viewBox. Optionally fill the background
    with `bg` (used for the social cards / app icons that need an opaque
    rounded square).
    """
    bg_layer = ""
    if bg is not None:
        if bg_corner_radius is None:
            bg_corner_radius = 0
        bg_layer = (
            f'<rect x="0" y="0" width="24" height="24" '
            f'rx="{bg_corner_radius}" ry="{bg_corner_radius}" fill="{bg}"/>'
        )
    return dedent(f"""\
        <?xml version="1.0" encoding="UTF-8"?>
        <svg xmlns="http://www.w3.org/2000/svg"
             viewBox="0 0 24 24"
             width="512" height="512"
             role="img"
             aria-label="TASMail">
          <title>TASMail</title>
          {bg_layer}
          {_envelope_paths(stroke, accent_at)}
        </svg>
    """).strip() + "\n"


def wordmark_svg(stroke: str, accent_at: str, wordmark_color: str, *,
                 bg: str | None = None) -> str:
    """
    240×56 horizontal lockup: 56×56 mark on the left, the word "TASMail" on the
    right with the cap-height aligned to the envelope.
    """
    bg_layer = ""
    if bg is not None:
        bg_layer = f'<rect x="0" y="0" width="240" height="56" fill="{bg}"/>'
    # Place the 24×24 mark inside a 56×56 viewport via translate+scale, then
    # set the wordmark to the right of it.
    return dedent(f"""\
        <?xml version="1.0" encoding="UTF-8"?>
        <svg xmlns="http://www.w3.org/2000/svg"
             viewBox="0 0 240 56"
             width="240" height="56"
             role="img"
             aria-label="TASMail">
          <title>TASMail</title>
          {bg_layer}
          <g transform="translate(2,4) scale(2)">
            {_envelope_paths(stroke, accent_at)}
          </g>
          <text x="64" y="38"
                font-family="'Inter','SF Pro Display','Segoe UI',system-ui,sans-serif"
                font-size="28"
                font-weight="800"
                letter-spacing="-0.6"
                fill="{wordmark_color}">TASMail</text>
        </svg>
    """).strip() + "\n"


def write(name: str, content: str) -> Path:
    path = ROOT / name
    path.write_text(content)
    return path


def main() -> None:
    artefacts = []

    # ---- primary mark (transparent bg) ----------------------------------------
    artefacts.append(write("logo-primary.svg",
                           primary_svg(stroke=CHARCOAL, accent_at=TEAL)))
    artefacts.append(write("logo-dark.svg",
                           primary_svg(stroke=WHITE, accent_at=TEAL)))
    artefacts.append(write("logo-mono-black.svg",
                           primary_svg(stroke=CHARCOAL, accent_at=CHARCOAL)))
    artefacts.append(write("logo-mono-white.svg",
                           primary_svg(stroke=WHITE, accent_at=WHITE)))

    # ---- mark with rounded-square brand backdrop (used by app icons + social) -
    artefacts.append(write("logo-tile-blue.svg",
                           primary_svg(stroke=WHITE, accent_at=TEAL,
                                       bg=BLUE, bg_corner_radius=4.5)))
    artefacts.append(write("logo-tile-charcoal.svg",
                           primary_svg(stroke=WHITE, accent_at=TEAL,
                                       bg=CHARCOAL, bg_corner_radius=4.5)))

    # ---- wordmark lockups -----------------------------------------------------
    out = ROOT.parent / "wordmark"
    out.mkdir(parents=True, exist_ok=True)
    (out / "wordmark-light.svg").write_text(
        wordmark_svg(stroke=CHARCOAL, accent_at=TEAL, wordmark_color=CHARCOAL)
    )
    (out / "wordmark-dark.svg").write_text(
        wordmark_svg(stroke=WHITE, accent_at=TEAL, wordmark_color=WHITE,
                     bg=CHARCOAL)
    )
    (out / "wordmark-blue.svg").write_text(
        wordmark_svg(stroke=BLUE, accent_at=TEAL, wordmark_color=BLUE)
    )
    artefacts += [out / "wordmark-light.svg", out / "wordmark-dark.svg", out / "wordmark-blue.svg"]

    print(f"Wrote {len(artefacts)} SVG variants:")
    for a in artefacts:
        print(f"  {a}")


if __name__ == "__main__":
    main()
