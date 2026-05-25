# Yappy iOS — Siri Shortcuts recipes

Yappy registers the `yappy://` URL scheme. Anything iOS does via "Open URL" can drive it.

## Hey Siri, read my clipboard

1. Open the **Shortcuts** app.
2. Tap **+** (new shortcut).
3. Add action: **Get Clipboard**.
4. Add action: **URL** — set the URL field to `yappy://shared`.
5. Add action: **Open URLs**.
6. Tap the settings icon at the bottom → rename to "Read clipboard with Yappy" → toggle **Add to Siri**.
7. Say "Hey Siri, read clipboard with Yappy" — Yappy opens, sees the pasteboard, fires the launch banner.

## Hey Siri, read this article

1. New shortcut.
2. Action: **URL** — set the URL field to a `yappy://shared?url=https://example.com/article` template.
3. Use **Ask for Input** if you want to prompt; otherwise hardcode the URL.
4. Save → Add to Siri.

## Home screen long-press menu

Long-press the Yappy icon. You'll see:
- **Read clipboard** — equivalent to "Hey Siri, read my clipboard"
- **Resume last** — opens the last document
- **Open document** — file picker

These are registered in `Info.plist` (`UIApplicationShortcutItems`); they map to the same `yappy://action/*` URLs.

## Sharing other apps' content into Yappy

Already wired without a shortcut. In any iOS app:
- Tap **Share** → **Yappy** in the share sheet. URLs get fetched + defuddle-extracted; text goes straight to TTS.
