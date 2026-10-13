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
npm run control -- use-codex
npm run control -- use-codex-speech
npm run control -- use-local-speech
npm run control -- dashboard
npm run control -- show
npm run control -- hide
npm run control -- prompt "Answer directly and keep responses concise."
```

`import-codex` reads `~/.codex/config.toml`, finds the selected provider,
uses its `base_url`, `env_key` and model, and sends the configuration to the
running app. The credential remains in the local request and Pluely Clean's
local storage.

`use-codex` selects a local provider that runs each response through
`codex exec` with an ephemeral, read-only session. It defaults to
`gpt-6.1-sol`; set `PLUELY_CODEX_MODEL` to override it.

`use-codex-speech` selects the transcription endpoint from the same Codex
provider configuration and reuses its credential. It defaults to
`gpt-transcribe`; set `PLUELY_CODEX_SPEECH_MODEL` to override it.

`use-local-speech` selects the loopback-only local Whisper service bundled
with this installation. Audio remains on the Mac and no speech API key is
required.

## macOS Calendar interview context

The app checks macOS Calendar once a minute for an interview that is currently
running or starts within 24 hours. An event is treated as an interview when its
title or calendar name contains `interview`, `recruiter screen`,
`technical screen` or `hiring manager`, or its notes contain interview
metadata.

For reliable context, add this compact block to the Calendar event notes:

```text
[interview]
as: Senior Product Engineer
timezone: America/New_York
resume: US product engineer v3
```

The fields are optional. The app appends available values, event time and
participants to the active system prompt without overwriting the base prompt.

Codex can also send a raw command:

```bash
npm run control -- send '{"type":"reload_settings"}'
```

Supported command types are:

- `configure_ai_provider`
- `configure_stt_provider`
- `set_system_prompt`
- `set_interview_context`
- `reload_settings`
- `open_dashboard`
- `show_overlay`
- `hide_overlay`
- `toggle_overlay`
