# Mobile distribution runbook — Play Store + Huawei AppGallery

**Owner:** Mobile / DevOps
**Status:** Live — used for every TASMail Mobile release
**Related:** TMAIL-56 (GAP-M-008), TMAIL-47 (closed beta), TMAIL-49 (MOBILE-PLATFORM-DECISION.md)

This runbook is the canonical procedure for shipping `mobile/` to end users.
It covers:

1. [Account setup](#1-account-setup) — Google Play Console + Huawei AppGallery Connect
2. [Versioning strategy](#2-versioning-strategy)
3. [Keystore ceremony](#3-keystore-ceremony) — one-time, never repeats
4. [Fastlane setup](#4-fastlane-setup) — local + CI
5. [iOS (deferred)](#5-ios-deferred)
6. [Release procedure](#6-release-procedure) — internal → closed beta → production
7. [Store assets](#7-store-assets) — screenshots, icon, feature graphic
8. [Analytics decision](#8-analytics-firebase-vs-matomo)
9. [Submission checklists](#9-submission-checklists)
10. [Troubleshooting](#10-troubleshooting)

---

## 1. Account setup

### Google Play Console (Android)

| Item | Action |
|---|---|
| Account type | **Organisation** (not Personal — required for company-name display) |
| Legal name | Tech at Scale Ltd (matches Ghana RoC registration — TMAIL-43) |
| Fee | $25 USD one-time, via Visa/Mastercard. Settle from Tech at Scale operations card. |
| Verification | D-U-N-S Number lookup at https://www.dnb.com/duns-number/lookup.html — if Tech at Scale doesn't have one, request a free one (takes 5–10 business days). |
| Tax & banking | Ghana TIN, GCB Bank settlement account (Tech at Scale main). Required to monetise — required even for free apps if any in-app purchases or subscriptions exist. |
| Console URL | https://play.google.com/console |

**Setup checklist:**

- [ ] Create the org account at https://play.google.com/console/signup
- [ ] Verify D-U-N-S
- [ ] Add `dominic@techatscale.io` as **Admin**
- [ ] Add `ops@techatscale.io` as **Release Manager** (no financial)
- [ ] Generate an OAuth service-account JSON for fastlane: Setup → API access → Choose a project → Create service account → Grant *Release Manager* role → Download JSON → save to 1Password as `Play Store Service Account JSON`
- [ ] Drop the JSON at `mobile/android/fastlane/play-store-credentials.json` (gitignored)

### Huawei AppGallery Connect

| Item | Action |
|---|---|
| Account type | **Enterprise developer** |
| Fee | **Free** (Huawei does not charge developer registration fees) |
| Verification | Business license (Ghana RoC certificate from TMAIL-43) + bank statement |
| Lead time | 1–3 business days (Huawei staff review the docs) |
| Console URL | https://developer.huawei.com/consumer/en/service/josp/agc/index.html |

**Setup checklist:**

- [ ] Create the Huawei developer account at https://developer.huawei.com/consumer/en
- [ ] Upload business license + ID
- [ ] After approval, create a new app in AppGallery Connect
- [ ] Note down `App ID` (e.g. `103456789`) and `Package Name` (must match Android `applicationId`: `io.techatscale.tasmail_mobile`)
- [ ] Create an API client: My Projects → Project Settings → API Management → Create — grant **Publish API** scope. Note `Client ID` and `Client Secret`.
- [ ] Store all four values (App ID, Package Name, Client ID, Client Secret) in 1Password as `Huawei AppGallery Credentials`

### Why Huawei matters for Ghana

Per the Ghana market research in `docs/BUSINESS-VALIDATION-GHANA.md`, Huawei + Honor devices have ~12–18% of the smartphone market depending on city. Many of those devices either ship without Google Play Services (post-May 2019 trade restrictions) or have it disabled in carrier ROMs. Distributing **only** to Play Store cuts that segment off entirely. The trade-off (extra ~30 min/release to upload to a second store) is worth it.

---

## 2. Versioning strategy

Mobile versioning is **independent** of the backend. Backend ships continuously; mobile cuts a release roughly every 2–3 weeks.

| Component | Source | Format |
|---|---|---|
| `versionName` (visible to user) | `version:` line in `mobile/pubspec.yaml` | semver `MAJOR.MINOR.PATCH` |
| `versionCode` (Android internal) | The `+N` suffix in `mobile/pubspec.yaml` | monotonically increasing integer |
| `CFBundleVersion` (iOS internal) | The `+N` suffix in `mobile/pubspec.yaml` | monotonically increasing integer |

**Rules:**

| Change | Bump |
|---|---|
| Bug fix only | PATCH (1.2.3 → 1.2.4) + `versionCode` |
| New user-visible feature | MINOR (1.2.3 → 1.3.0) + `versionCode` |
| Breaking change / forced re-onboarding | MAJOR (1.2.3 → 2.0.0) + `versionCode` |
| Re-build same version (rare) | `versionCode` only, `versionName` unchanged |

**`versionCode` never goes down.** Play Store rejects an upload with a `versionCode` ≤ the highest one previously published on **any** track. Keep a single increasing counter across `internal` → `beta` → `production`.

### Bumping the version

Edit `mobile/pubspec.yaml`:

```yaml
# Before
version: 1.2.3+15

# After (bug fix)
version: 1.2.4+16

# After (new feature)
version: 1.3.0+17
```

Then commit with a message that references the PM ticket(s) bundled into the
release (e.g. `chore(mobile): bump to 1.3.0+17 — TMAIL-141, TMAIL-142, TMAIL-149`).

The release script picks these up automatically:

```bash
mobile/scripts/release.sh 1.3.0 17
```

---

## 3. Keystore ceremony

**This is the single most security-critical step.** Lose the keystore →
**you can never publish another update to this Play Store listing again**.
Google does not have a recovery process — you would have to publish a new app
under a new package name and lose your user base.

### Generate

Once, on a trusted machine:

```bash
mkdir -p ~/keys
keytool -genkey -v \
    -keystore ~/keys/tasmail-upload.jks \
    -keyalg RSA -keysize 2048 -validity 10000 \
    -alias tasmail-upload \
    -storetype JKS
```

Fields:

- **Common Name:** `TASMail`
- **Org Unit:** `Mobile`
- **Org:** `Tech at Scale Ltd`
- **Locality:** `Accra`
- **State:** `Greater Accra`
- **Country:** `GH`
- **Store password:** Generate with `openssl rand -base64 24` — save in 1Password
- **Key password:** Same as store password (operationally simpler)

### Store

1. Upload `tasmail-upload.jks` as a **file attachment** to a new 1Password item: vault `TASMail Mobile`, item title `Android Upload Keystore`.
2. Save the store/key passwords as two fields on the same 1Password item.
3. **Delete** `~/keys/tasmail-upload.jks` from disk after upload. Always re-fetch from 1Password before signing.

### Enrol Play App Signing

In Play Console → App signing → opt in to Google Play App Signing. Upload the
keystore (Google calls this the "upload key"). Google then generates and
*holds* the **distribution key** — that is the one users' devices verify
against. Even if we lose the upload key, Google can issue a new one without
breaking installs.

> **Do this on day one.** Switching to Play App Signing after the fact is
> harder (requires Google support tickets).

### Configure local builds

```bash
# On the dev/CI machine:
op read "op://TASMail Mobile/Android Upload Keystore/tasmail-upload.jks" \
    > ~/keys/tasmail-upload.jks
chmod 600 ~/keys/tasmail-upload.jks

# Copy + edit key.properties
cd ~/Documents/code/project-email-service/mobile/android
cp key.properties.example key.properties
# Edit key.properties with the values from 1Password
```

`build.gradle.kts` will pick `key.properties` up automatically — see
`mobile/android/app/build.gradle.kts` for the gradle-side handling.

---

## 4. Fastlane setup

### Install (one-time, per machine)

```bash
# Ruby (use rbenv or asdf — system Ruby on Ubuntu is too old)
asdf install ruby 3.2.2
asdf global ruby 3.2.2

# Fastlane via bundler
cd ~/Documents/code/project-email-service/mobile/android
gem install bundler
cat > Gemfile <<EOF
source "https://rubygems.org"
gem "fastlane"
EOF
bundle install
bundle exec fastlane install_plugins   # picks up Pluginfile (Huawei + changelog)
```

The same procedure works for `mobile/ios/` on macOS.

### Lanes

All lanes live in `mobile/android/fastlane/Fastfile`. Run from `mobile/android/`:

| Lane | Command | Effect |
|---|---|---|
| Internal testing | `bundle exec fastlane android internal` | Builds signed AAB, uploads to Play Console **Internal testing** track. Status: `draft` — you still click "Submit" in console. |
| Closed beta | `bundle exec fastlane android beta` | Builds signed AAB, uploads to **Closed beta** track. Status: `completed` — auto-rolls to beta testers. |
| Production | `bundle exec fastlane android production` | Builds signed AAB, uploads to **Production** track. Status: `inProgress` with **10% rollout** — bump to 100% in console after 48h of no crash spike. |
| Huawei AppGallery | `bundle exec fastlane android appgallery` | Builds signed APK, uploads to AppGallery Connect. **Does not submit for review** — that's a manual click in the AGC console. |

### Environment variables

| Var | Purpose | Source |
|---|---|---|
| `BUILD_NAME` | Sets `versionName`. Optional — defaults to "1.0.0" if omitted. | CLI or CI |
| `BUILD_NUMBER` | Sets `versionCode`. Required for production. | CLI or CI |
| `HUAWEI_CLIENT_ID` | AppGallery Connect API client | 1Password `Huawei AppGallery Credentials` |
| `HUAWEI_CLIENT_SECRET` | AppGallery Connect API secret | 1Password `Huawei AppGallery Credentials` |
| `HUAWEI_APP_ID` | The numeric AppGallery app ID | 1Password `Huawei AppGallery Credentials` |

For local runs, export them in your shell before invoking fastlane. For CI,
set them as encrypted secrets on the release runner — they should **never**
appear in PR-triggered jobs (only on `release-*` branches that require manual
push from a trusted human).

---

## 5. iOS (deferred)

Per `docs/MOBILE-PLATFORM-DECISION.md` (TMAIL-49), iOS launches **after**
Android stabilises. Reasons:

- Ghana smartphone market is ~94% Android per Statista
- iOS Apple Developer Program is $99/year USD vs Play's one-time $25
- TestFlight requires a Mac for builds — no current macOS CI runner

The iOS fastlane scaffolding (`mobile/ios/fastlane/`) exists so the work is
ready to pick up, but the lanes are not exercised in CI. When iOS launches:

1. Enrol Apple Developer Program at https://developer.apple.com/programs/
2. Create the bundle ID `io.techatscale.tasmail.mobile` in App Store Connect
3. Generate an App Store Connect API key (Users and Access → Keys), download
   the `.p8`, save to 1Password as `App Store Connect API Key`
4. Set up code signing via fastlane `match` (separate runbook TBD)
5. Run `bundle exec fastlane ios beta` from a macOS workstation

---

## 6. Release procedure

### 6.1 Cutting a release

```bash
# 1. Pull latest main
cd ~/Documents/code/project-email-service
git checkout main && git pull

# 2. Bump version in pubspec.yaml
# Edit mobile/pubspec.yaml — change "1.2.3+15" → "1.3.0+16"

# 3. Run tests + analyze (CI also does this on PR — this is belt-and-braces)
cd mobile
flutter analyze
flutter test

# 4. Commit the version bump
cd ..
git add mobile/pubspec.yaml
git commit -m "chore(mobile): bump to 1.3.0+16 — TMAIL-141, TMAIL-149"
SSH_AUTH_SOCK=/run/user/1000/gcr/ssh git push --no-verify

# 5. Build + upload to Play internal testing
cd mobile/android
BUILD_NAME=1.3.0 BUILD_NUMBER=16 bundle exec fastlane android internal

# 6. Smoke-test on a tester device for 24h via the internal track
#    (install link from Play Console → Internal testing → Testers)

# 7. Promote to closed beta
BUILD_NAME=1.3.0 BUILD_NUMBER=16 bundle exec fastlane android beta

# 8. After 5–7 days of beta with no Sentry spike, promote to production
BUILD_NAME=1.3.0 BUILD_NUMBER=16 bundle exec fastlane android production

# 9. Upload to AppGallery in parallel (do this any time after step 5)
HUAWEI_CLIENT_ID=... HUAWEI_CLIENT_SECRET=... HUAWEI_APP_ID=... \
    BUILD_NAME=1.3.0 BUILD_NUMBER=16 bundle exec fastlane android appgallery
# Then manually click "Submit for Review" in the AGC console.
```

### 6.2 Hotfix flow

For a P0 mobile crash:

1. Cherry-pick the fix onto a `hotfix-<versionCode>` branch
2. Bump PATCH + `versionCode`
3. Run fastlane lanes `internal` + `beta` simultaneously (skip 24h soak)
4. After 6h of clean Sentry, push to production at **100% rollout** (override the 10% default)
5. AppGallery upload + manual submit
6. Post-mortem within 5 working days, link from the original incident PM ticket

### 6.3 Rolling back a broken release

Play Store does not support a true rollback — you bump versionCode and ship a
"fixed" version with the prior good code. Steps:

1. Stop the rollout in Play Console → Production → Manage → Halt rollout
2. Revert the offending commit(s) on main
3. Bump PATCH + versionCode
4. Run the hotfix flow (6.2)

---

## 7. Store assets

### Required for Play Store

| Asset | Spec | Source |
|---|---|---|
| App icon | 512 × 512 PNG | `branding/build/app-icons/play-store/icon-512.png` |
| Feature graphic | 1024 × 500 JPEG | `branding/build/app-icons/play-store/feature-1024x500.jpg` |
| Phone screenshots (4–8) | 1080 × 1920 min, JPEG or PNG | Captured on a real Ghana-SIM device — see below |
| Privacy policy URL | Public HTTPS | `https://mail.techatscale.io/privacy-policy/mobile` (renders `docs/PRIVACY-POLICY-MOBILE.md`) |
| Short description | ≤ 80 chars | `mobile/android/fastlane/metadata/android/en-US/short_description.txt` |
| Full description | ≤ 4000 chars | `mobile/android/fastlane/metadata/android/en-US/full_description.txt` |
| Content rating questionnaire | IARC self-classification | Filled in console — TASMail is "Everyone" (no UGC moderation needed because email is private 1-to-1) |
| Data safety form | Lists data collected | Filled in console — see §9.1 |

### Required for Huawei AppGallery

Same as Play, plus:

| Asset | Spec |
|---|---|
| Hi-res app icon | 1024 × 1024 PNG |
| Phone screenshots (3–5 minimum) | 1080 × 1920 min, PNG only (no JPEG) |
| App introduction video | Optional, MP4 ≤ 30 MB, 16:9 |

### Capturing screenshots

Take real-device screenshots on:

1. **Tecno Camon** (entry tier, common in Ghana — represents 30% of market)
2. **Samsung Galaxy A-series** (mid tier)
3. **Pixel 7a / Honor 90** (high tier, also covers AppGallery)

Screens to capture (in this order):

1. Inbox after onboarding — shows realistic Gmail content
2. Composer mid-write with attachment chip
3. Message view with HTML email rendered
4. Search results
5. Settings — biometric unlock toggle visible
6. Offline state — "Offline. Changes will sync." banner

Store the raw captures in 1Password under `Play Store Assets`. **Do not commit
screenshots to git** — they bloat the repo and the source-of-truth is the
device captures, not pre-edited versions.

---

## 8. Analytics — Firebase vs Matomo

**Decision: Matomo, not Firebase.**

| | Firebase Analytics | Matomo |
|---|---|---|
| Data residency | Google US/EU servers | Self-hosted on `tas-src-1` |
| Ghana DPC compliance | Requires data-processing addendum | Already inside our hosting (TMAIL-44 satisfied trivially) |
| AppGallery compatibility | **Broken on Huawei devices without GMS** | Native HTTP — works everywhere |
| Cost | Free up to 500 events/sec | $0 (self-hosted) |
| Vendor lock-in | Hard — uses google-services.json wiring | Soft — just a `dio` POST |
| Already in our stack? | No | Yes — `tascim-web` uses Matomo |

The Flutter integration is a thin wrapper around `dio` posting to
`https://matomo.techatscale.io/matomo.php` — implementation pending under
TMAIL-XXX (file follow-up ticket when starting analytics work).

Firebase Crashlytics is also rejected for the same Huawei-compatibility reason.
Sentry already covers crash reporting via `sentry_flutter` on Android **and**
the Huawei-distributed APK (Sentry is HTTP-only, no GMS dependency).

---

## 9. Submission checklists

### 9.1 Google Play — first submission

- [ ] Account fully verified (D-U-N-S, tax, banking)
- [ ] App created in console, package name `io.techatscale.tasmail_mobile`
- [ ] Opted into Play App Signing
- [ ] Upload key generated and stored in 1Password
- [ ] First AAB uploaded to Internal testing
- [ ] **Store listing** complete — title, short desc, full desc, icon, feature graphic, ≥ 4 phone screenshots
- [ ] **Privacy policy URL** filled (`https://mail.techatscale.io/privacy-policy/mobile`)
- [ ] **Data safety form** filled — declare: email content (transmitted, not stored on TASMail servers — IMAP proxied), email metadata (cached locally, encrypted), name + email address (account), device identifiers (for push notifications). Mark all as "Required for app functionality", encryption in transit YES, deletion mechanism YES (account deletion).
- [ ] **Content rating** completed — IARC questionnaire, expect "Everyone"
- [ ] **Target audience** set — "13 and over"
- [ ] **Ads** declaration — "No, my app does not contain ads"
- [ ] **App access** — provide reviewer with a demo Gmail account for testing the BYOK flow (set up at `tasmail-play-reviewer@gmail.com`, credentials in 1Password)
- [ ] **News app** declaration — "No"
- [ ] **COVID-19 app** declaration — "No"
- [ ] **Government app** declaration — "No"
- [ ] **Financial features** declaration — "No"
- [ ] **Health features** declaration — "No"
- [ ] Closed testing track created and ≥ 12 testers added (Play requires 12+ testers for 14 days before allowing production)
- [ ] **All 12 testers opt in** via the opt-in URL
- [ ] After 14 days of active testing → request production access
- [ ] Production release with 10% rollout, monitor 48h, then 100%

### 9.2 Huawei AppGallery — first submission

- [ ] Enterprise account approved
- [ ] App created in AGC, package name matches Android `applicationId`
- [ ] API client credentials generated and stored
- [ ] APK uploaded via fastlane lane
- [ ] **App information** complete — name, brief, full description (Chinese translation NOT required for our region — Ghana market is en-GH)
- [ ] **Compliance** filled — privacy policy URL, data category form (similar shape to Play data safety)
- [ ] **Sensitive permission justifications** filled — for every permission in the AndroidManifest the AGC reviewer wants a sentence ("CAMERA: optional, for attaching photos to outgoing mail")
- [ ] **Submit for review** — turnaround is typically 1–3 business days

---

## 10. Troubleshooting

### Fastlane "Failed to refresh access token"

Service-account JSON expired or revoked. Regenerate at Play Console → Setup →
API access → service account → keys → Add key, replace
`play-store-credentials.json`, re-run.

### Play upload rejected — "Version code N has already been used"

You skipped or reused a versionCode. Bump again in `pubspec.yaml`, never go
backwards.

### Huawei upload rejected — "APK signature mismatch"

First AppGallery upload locked in the signing certificate. The keystore on the
upload machine doesn't match. Re-fetch the keystore from 1Password — make sure
you grabbed the same one used last time.

### "Your app has been removed for policy violation"

Most common cause for an email app: Play's "Sensitive permissions" review
flags `READ_CONTACTS` or `READ_MEDIA_IMAGES`. Reply via the policy ticket with
the per-permission justification (matches what we filled for AppGallery in §9.2).

### `flutter build appbundle --release` fails with "Keystore was tampered with"

Wrong store password. Re-check `key.properties` against 1Password — common
mistake is whitespace at end of password field.

### Sentry not capturing the production AppGallery build

Sentry release-tag mismatch — the AppGallery APK has the same versionCode as
Play's AAB but the build flavour differs. Either set `release` explicitly in
`sentry_flutter.init()` to `versionName+versionCode` (no build flavour
appended) or upload a separate AppGallery sourcemap per release.

---

## References

- `mobile/android/app/build.gradle.kts` — gradle-side signing config
- `mobile/android/key.properties.example` — keystore credentials template
- `mobile/android/fastlane/Fastfile` — release lanes
- `mobile/scripts/release.sh` — local build helper
- `.github/workflows/mobile-android-build.yml` — PR sanity check
- `docs/PRIVACY-POLICY-MOBILE.md` — privacy policy source (rendered at `/privacy-policy/mobile`)
- `docs/MOBILE-PLATFORM-DECISION.md` — TMAIL-49 ADR for Flutter + Android-first
- `docs/BETA-LAUNCH-RUNBOOK.md` — TMAIL-47 closed-beta operational playbook
- `branding/BRAND.md` — palette, mark, and where store icons live

### External

- Google Play Console: https://play.google.com/console
- Huawei AppGallery Connect: https://developer.huawei.com/consumer/en/service/josp/agc/index.html
- Fastlane docs (supply): https://docs.fastlane.tools/actions/supply/
- Fastlane Huawei plugin: https://github.com/karumi/fastlane-plugin-huawei_appgallery_connect
- Play App Signing: https://support.google.com/googleplay/android-developer/answer/9842756
- Play closed-testing 12-tester requirement: https://support.google.com/googleplay/android-developer/answer/14151465
