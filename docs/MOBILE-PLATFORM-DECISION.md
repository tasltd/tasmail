# Mobile Platform Decision: Flutter for the Ghana Market

**Status:** Accepted · **Date:** 2026-04-15 · **PM ticket:** [TMAIL-49](https://cim.techatscale.io/projects/TMAIL/issues/TMAIL-49) · **Implements:** PRD §NG2 (native mobile apps, v2.0)

## Context

TASMail ships a React 19 PWA that already covers the desktop/mobile-web surface
for BYOK accounts. PRD §NG2 ("Mobile native apps") is on the v2.0 roadmap with
the note *"React PWA covers mobile; native apps are v2.0+"*. We need a single
mobile codebase that hits Ghana's installed base without doubling team headcount.

Ghana market constraints (see `docs/BUSINESS-VALIDATION-GHANA.md`):

- 42M+ mobile connections, ~128% penetration — Android-dominant
- Mid-range hardware is the norm — 2 GB RAM, 720p, ARM v7/v8
- Many users rely on mobile money rails (MTN MoMo, Vodafone Cash) — UX must
  feel native, not webview-wrapped
- Local language UX matters — Twi, Ewe, Ga, Hausa are first-class
- Huawei AppGallery presence is non-trivial in West Africa

## Decision

**Flutter (Dart, stable channel)** for Android + iOS, with Huawei AppGallery
support via HMS Core when we cut the v2.0 release.

Minimum supported target: **Android 8 (API 26), 2 GB RAM, 720p**.

## Alternatives Considered

| Option | Verdict | Why |
|---|---|---|
| **Flutter** | ✅ Chosen | Single codebase across Android+iOS+Huawei, AOT-compiled ARM is fast on low-end hardware, ~8 MB base APK, mature offline storage (Hive/Isar), Material 3 widgets ship out of the box. Dart team has stable LTS cadence. |
| React Native | ❌ Rejected | ~15 MB base APK, JS bridge cost on low-RAM devices, weaker Huawei tooling, would duplicate UI patterns from the React SPA only nominally — most components don't transfer because RN ≠ DOM. |
| Native Android + native iOS (Kotlin + Swift) | ❌ Rejected | Doubles engineering cost and shipping cadence; the team is one mobile engineer. Doesn't pay off for a webmail UI where 90% of logic is REST/IMAP shaped, not platform shaped. |
| PWA only (status quo) | ❌ Rejected | Already done. Doesn't unlock background push, biometric auth, or AppGallery distribution. Users on cheap Android devices report battery drain when the PWA is the primary mail client. |
| Kotlin Multiplatform Mobile | ❌ Rejected | UI layer still has to be written twice (Compose + SwiftUI); slower hiring market in Ghana; tooling churn through 2025. Revisit at v3.0 if Compose Multiplatform matures. |

## Rationale Detail

1. **Single codebase, two stores (three with Huawei)** — one Flutter engineer
   covers what would otherwise need a Kotlin + Swift pair. Critical given the
   team size in `docs/PROJECT-MEMBERS.md`.
2. **APK size matters in Ghana** — many users are on metered data and pay per MB
   for installs. ~8 MB beats ~15 MB by a margin users actually notice.
3. **Native ARM AOT** — Flutter compiles Dart to ARM machine code. No JS bridge,
   no Hermes interpreter, smoother scroll on 2 GB devices.
4. **Offline-first fits BYOK** — Hive/Isar give us reliable local stores for
   draft queue, sync checkpoints, and message cache. Critical for unstable 3G.
5. **Localization** — `flutter_localizations` is mature; we ship Twi, Ewe, Ga,
   and Hausa alongside English on day one.
6. **Material 3 alignment** — matches the alt-UI shadcn/Tailwind direction of
   the web SPA, so brand carries across surfaces without bespoke design tokens.

## Tradeoffs Accepted

- Dart is a smaller talent pool than Kotlin or TS — mitigated by single-engineer
  ownership and good docs (the existing 27 lib files + 12 test files document
  the patterns).
- iOS builds require a Mac in CI — accepted; one Apple Silicon runner suffices
  for the release cadence we plan.
- Skia rendering doesn't reuse system text on iOS — minor accessibility
  consideration; the impact tests clean on VoiceOver in our smoke runs.

## Consequences

### Implemented in this decision (already shipped in `mobile/`)

| Area | Location |
|---|---|
| App entry point + provider tree | `mobile/lib/main.dart` |
| API client (Dio over the same REST surface as the SPA) | `mobile/lib/api/api_client.dart` |
| Auth + mail providers | `mobile/lib/providers/{auth,mail}_provider.dart` |
| Screens (login, home, inbox, message, compose, search, settings, folders) | `mobile/lib/screens/**` |
| Services (push, sync, offline draft queue, biometric) | `mobile/lib/services/**` |
| Localizations | `mobile/lib/l10n/app_localizations{,_en,_tw,_ee,_gaa,_ha}.dart` |
| Models (auth, email, sync checkpoint) | `mobile/lib/models/**` |
| Tests (39 cases across 12 spec files) | `mobile/test/**` |

Build commands live in the project's root `CLAUDE.md` under *Mobile App*.

### Follow-on tickets (in PM, already tracked)

- TMAIL-50 — FCM/APNs push notification service (In Review)
- TMAIL-51 — Offline-first sync protocol (In Review)
- TMAIL-52 — Mobile-optimized API endpoints (In Review)
- TMAIL-142 — Biometric authentication (Backlog)
- Huawei AppGallery publishing flow — to be opened against HMS push parity

### Out of scope

- Web push for the SPA is unchanged — this decision is mobile only.
- Desktop wrappers (Electron, Tauri) are explicitly *not* a future direction;
  the PWA covers desktop.

## References

- `docs/PRD.md` §NG2 — mobile native apps roadmap entry
- `docs/BUSINESS-VALIDATION-GHANA.md` — market sizing, device mix, payment rails
- `docs/PROJECT-MEMBERS.md` — team capacity
- Root `CLAUDE.md` → *Mobile App (`mobile/`)* — build/test commands
- PM ticket TMAIL-49 — decision audit trail and review history
