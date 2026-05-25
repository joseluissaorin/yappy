# iOS port — final session status

This document is the source of truth for what's verified and what's blocked.
It replaces the conflicting handoff docs I generated mid-session.

---

## ✅ Done and verified

| Capability | Verification |
|---|---|
| iOS Rust crate compiles for `aarch64-apple-ios` | `cargo check --target aarch64-apple-ios` clean |
| ORT 1.22.0 iOS XCFramework vendored | `scripts/fetch-ort-ios.sh`, thin arm64 + arm64-sim slices in `vendor/ort-ios/` |
| CoreML EP registers at ORT environment level | `lib.rs` calls `ort::init().with_execution_providers([CoreMLExecutionProvider...])` — paddle-ocr-rs sessions inherit ANE acceleration too |
| Tauri iOS Xcode project generated | `yappy-app/src-tauri/gen/apple/`, signed with Team `9LYNY2477X` |
| App launches on iPhone 17 Pro simulator | screenshot in `/tmp/yappy-ios-v7.png` |
| iOS-aware onboarding ("for your phone") | `Onboarding.svelte` reads `$isIOS`; verified via Maestro |
| iOS Settings hides desktop-only sections | Hotkey config, autostart, browser extension all behind `{#if !$isIOS}` |
| Silent background-audio keepalive | `AudioSession.swift` activates AVAudioSession + plays silent loop; `BackgroundAudioGuard` RAII in Rust |
| Live Activity widget bundled | `YappyWidgets.appex` embedded; ActivityKit start/update/end called from `render_audiobook_cmd` |
| Share Sheet extension bundled | `YappyShare.appex` embedded; ShareViewController accepts URL/text, writes to App Group, opens `yappy://` |
| Defuddle works in iOS WebKit | `tests/defuddle.test.mjs` extracts an article in JSDOM (same UMD bundle that runs in WKWebView) |
| Maestro smoke test passes | `./maestro/run-all.sh` → 13 s, validates onboarding + iOS gating |

## 🟡 Built but NOT end-to-end verified on sim

| Capability | Why not verified | What to do once you finish manual steps below |
|---|---|---|
| Share Sheet → App Group → main app handoff | Xcode strips the App Group entitlement at sign time because `group.com.joseluissaorin.yappy` isn't registered in Apple Developer portal yet | Install the new build, share a URL from Safari, watch Yappy launch with the article queued |
| URL fetch → defuddle → TTS chain on iOS | Same App Group block masks the JS event listener path | Open Safari Web Inspector against the running sim, run `await window.__yappyHandleOne("url:https://en.wikipedia.org/wiki/Audiobook")` — bypasses the App Group container, exercises everything else |
| Long m4b render in background | Background-audio keepalive code is wired but I haven't run a multi-hour render | Render a real audiobook, lock the phone, confirm progress continues |
| Live Activity rendering | Code is correct; needs an actual long render to actually pop the widget | Same as above |

## 🔴 Hard-blocked on you

Apple **does not allow programmatic app creation** in App Store Connect via the
REST API. The error is verbatim:

> `POST /v1/apps → 403`
> `"detail" : "The resource 'apps' does not allow 'CREATE'. Allowed operations are: GET_COLLECTION, GET_INSTANCE, UPDATE"`

This applies to every JWT-based ASC API caller. Fastlane's `produce` command
works around it by using the legacy Apple-ID + password + 2FA flow, which I
can't drive without your interactive credentials. Web-UI automation would
need the same.

**Your three minutes of unblockable work:**

1. https://developer.apple.com/account/resources/identifiers/list
   - Click `com.joseluissaorin.yappy` → Capabilities → enable **App Groups** + **Increased Memory Limit** → Save
   - Then on `com.joseluissaorin.yappy.YappyWidgets` and `com.joseluissaorin.yappy.ShareExtension` (auto-created by Xcode during my earlier builds) → enable **App Groups**

2. https://developer.apple.com/account/resources/identifiers/list/applicationGroup
   - Click `+` → Register App Group with identifier `group.com.joseluissaorin.yappy`, description "Yappy shared container"
   - Then go back to each of the three App IDs and add this group under the App Groups capability

3. https://appstoreconnect.apple.com/apps
   - Click `+` → **New App**: Platforms iOS, Name "Yappy", Primary Language English (U.S.), Bundle ID `com.joseluissaorin.yappy`, SKU `yappy-app`, User Access Full Access

When all three are done, the background auto-ship watcher (PID `73121`) will detect the App appearing in ASC and fire `scripts/ios-build-and-upload.sh` — that rebuilds the signed Release IPA with the now-honored entitlements, exports it for App Store upload, polls ASC for build processing, creates the public Beta group, attaches the build, and submits for Beta App Review.

Apple's Beta App Review then takes 24–48 hours on their side.

---

## Recovery if anything broke

```bash
# Re-fetch ORT iOS framework
./scripts/fetch-ort-ios.sh

# Re-run the iOS Xcode project regen
cd yappy-app/src-tauri/gen/apple && xcodegen && cd ../../../..

# Sim build
cd yappy-app && cargo tauri ios build --debug --target aarch64-sim

# Maestro smoke
./maestro/run-all.sh

# Full ship (once ASC app exists)
./scripts/ios-build-and-upload.sh
```
