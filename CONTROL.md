# Pluely Clean control layer

Pluely Clean listens only on `127.0.0.1:37421`. It creates a random bearer
token on first launch and stores it in the app data directory as
`control-token` with owner-only permissions on Unix systems.

The control layer has one command interface:

```text
POST /v1/commands
Authorization: Bearer <local token>
Content-Type: application/json
```

The bundled CLI reads the token without printing it:

```bash
npm run control -- health
npm run control -- import-codex
npm run control -- dashboard
npm run control -- show
npm run control -- hide
npm run control -- prompt "Answer directly and keep responses concise."
```

`import-codex` reads `~/.codex/config.toml`, finds the selected provider,
uses its `base_url`, `env_key` and model, and sends the configuration to the
running app. The credential remains in the local request and Pluely Clean's
local storage.

Codex can also send a raw command:

```bash
npm run control -- send '{"type":"reload_settings"}'
```

Supported command types are:

- `configure_ai_provider`
- `configure_stt_provider`
- `set_system_prompt`
- `reload_settings`
- `open_dashboard`
- `show_overlay`
- `hide_overlay`
- `toggle_overlay`
