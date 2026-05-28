# Play Store metadata

Files in this directory feed `fastlane supply` (the Play Store upload tool).
They are the **source of truth** for the listing — the Play Console UI is
treated as read-only.

## Directory layout

```
metadata/
└── android/
    └── en-US/
        ├── title.txt              ≤ 30 chars
        ├── short_description.txt  ≤ 80 chars
        ├── full_description.txt   ≤ 4000 chars
        ├── video.txt              (optional YouTube URL)
        ├── changelogs/
        │   └── default.txt        ≤ 500 chars; per-versionCode files override
        └── images/                NOT committed — see below
            ├── icon/
            ├── featureGraphic/
            ├── phoneScreenshots/
            ├── sevenInchScreenshots/
            └── tenInchScreenshots/
```

## Images

Screenshots, the feature graphic, and the high-res icon are **NOT** committed
to git (they're typically 1–3 MB each × 8 screenshots × 3 device classes ×
N locales = bloat). Instead they live in 1Password (vault: "TASMail Mobile",
item: "Play Store Assets") and are pulled into `images/` only at release time.

To pull the assets:

```bash
# From mobile/android/fastlane/metadata/android/en-US/
mkdir -p images
op read "op://TASMail Mobile/Play Store Assets/images.tar.gz" | tar -xz -C images
```

Required image specs (Google Play 2026):

| Asset                | Size       | Format         | Count |
|----------------------|------------|----------------|-------|
| App icon             | 512 × 512  | 32-bit PNG     | 1     |
| Feature graphic      | 1024 × 500 | JPEG / 24-bit  | 1     |
| Phone screenshots    | 1080 × 1920 (min) | JPEG / PNG | 4–8 |
| 7-inch tablet        | 1200 × 1920 (min) | JPEG / PNG | 1–8 (optional) |
| 10-inch tablet       | 1920 × 1200 (min) | JPEG / PNG | 1–8 (optional) |

Capture screenshots on a real Ghanaian-SIM device for authenticity
(see runbook §7).

## Adding a new locale

```bash
cp -r en-US tw-GH   # Twi
cp -r en-US ee-GH   # Ewe
# ... edit each *.txt file in place
```

Then re-run `bundle exec fastlane android beta` — supply will upload all
locales in one call.

## See also

- `docs/MOBILE-DISTRIBUTION-RUNBOOK.md` — full release procedure
- `docs/PRIVACY-POLICY-MOBILE.md` — privacy policy text the listing links to
