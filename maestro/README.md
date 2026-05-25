# Maestro flows for Yappy iOS

End-to-end UI tests against the iOS Simulator. Maestro picks an element by
its accessibility text (or `id` for native controls) — for Tauri WKWebView
content, the rendered text in the DOM IS the accessibilityText, so Svelte
copy doubles as a test selector.

## Setup

Install Maestro once: https://maestro.dev/docs/getting-started

```bash
curl -Ls https://get.maestro.mobile.dev | bash
```

XCUITest driver auto-installs on first run; subsequent runs are fast.

## Run a flow

```bash
# List booted simulators
xcrun simctl list devices booted

# Run one flow against a specific simulator
maestro --device E08E324E-5C95-433C-88D6-F63B652DE9E8 \
        test maestro/flows/onboarding.yaml

# Run all flows in the folder, sorted alphabetically
maestro --device E08E324E-5C95-433C-88D6-F63B652DE9E8 \
        test maestro/flows/
```

## Flows

| File | What it does |
|---|---|
| `smoke.yaml` | Full first-launch journey: onboarding iOS-aware copy + "get started" → install-voices card → Settings iOS-gating (hotkey/autostart/browser-extension absent). One flow, 13s, exercises every iOS-specific UI path. |

Recommended invocation:

```bash
./maestro/run-all.sh
```

Wraps `simctl uninstall` + `simctl install` before each test pass —
necessary because Maestro's `clearState: true` does not actually wipe
iOS UserDefaults, so `first_launch_done` persists across test runs and
masks the onboarding flow.
