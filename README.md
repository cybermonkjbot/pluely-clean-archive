# Pluely Clean

Pluely Clean is a distraction-free local fork of the final open-source Pluely
release, v0.1.9.

This fork keeps the overlay, chats, prompts, screenshots, audio controls,
custom AI providers and custom speech providers. It removes licence checkout,
upgrade, subscription, usage-meter and promotional surfaces. The official
hosted API is disabled; the app uses providers configured by the person running
it.

The app has its own product name, bundle identifier and data directory, so it
can be installed beside an official Pluely release without sharing state.

## Local control

The local control layer lets Codex or another process configure providers,
prompts and window visibility through an authenticated loopback endpoint. See
[CONTROL.md](CONTROL.md).

## Development

```bash
npm install
npm run tauri dev
```

Production packaging uses the standard Tauri command:

```bash
npm run tauri build
```

## Licence and attribution

This fork is based on Pluely v0.1.9, originally published by Srikanth Nani
under GPL-3.0. The original licence is retained in [LICENSE](LICENSE).
