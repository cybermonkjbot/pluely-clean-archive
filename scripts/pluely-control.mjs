#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const address = process.env.PLUELY_CLEAN_CONTROL_URL || "http://127.0.0.1:37421";
const command = process.argv[2];

if (!command) {
  fail("Usage: npm run control -- <health|show|hide|toggle|dashboard|prompt|send|import-codex|use-codex|use-codex-speech|use-local-speech>");
}

if (command === "health") {
  const response = await fetch(`${address}/v1/health`);
  process.stdout.write(`${await response.text()}\n`);
  process.exit(response.ok ? 0 : 1);
}

const token = fs.readFileSync(resolveTokenPath(), "utf8").trim();
let payload;

switch (command) {
  case "show":
    payload = { type: "show_overlay" };
    break;
  case "hide":
    payload = { type: "hide_overlay" };
    break;
  case "toggle":
    payload = { type: "toggle_overlay" };
    break;
  case "dashboard":
    payload = { type: "open_dashboard" };
    break;
  case "prompt":
    payload = { type: "set_system_prompt", prompt: process.argv.slice(3).join(" ") };
    if (!payload.prompt) fail("The prompt command requires prompt text.");
    break;
  case "send":
    payload = JSON.parse(process.argv.slice(3).join(" "));
    break;
  case "import-codex":
    payload = createCodexProviderCommand();
    break;
  case "use-codex":
    payload = createCodexHarnessCommand(token);
    break;
  case "use-codex-speech":
    payload = createCodexSpeechCommand();
    break;
  case "use-local-speech":
    payload = createLocalSpeechCommand(token);
    break;
  default:
    fail(`Unknown control command: ${command}`);
}

function createCodexHarnessCommand(token) {
  const model = process.env.PLUELY_CODEX_MODEL || "gpt-6.1-sol";
  return {
    type: "configure_ai_provider",
    provider: {
      id: "codex-local-harness",
      curl: [
        `curl ${address}/v1/codex/chat/completions`,
        '-H "Content-Type: application/json"',
        '-H "Authorization: Bearer {{API_KEY}}"',
        `-d '${JSON.stringify({
          model: "{{MODEL}}",
          messages: [
            { role: "system", content: "{{SYSTEM_PROMPT}}" },
            { role: "user", content: "{{TEXT}}" },
          ],
        })}'`,
      ].join(" \\\n+  "),
      streaming: false,
      responseContentPath: "choices[0].message.content",
    },
    variables: { api_key: token, model },
    select: true,
  };
}

const response = await fetch(`${address}/v1/commands`, {
  method: "POST",
  headers: {
    authorization: `Bearer ${token}`,
    "content-type": "application/json",
  },
  body: JSON.stringify(payload),
});
const responseBody = await response.text();
if (!response.ok) fail(`Control request failed (${response.status}): ${responseBody}`);
process.stdout.write(`${responseBody}\n`);

function createCodexProviderCommand() {
  const config = readCodexProviderConfig();
  const endpoint = `${config.baseUrl.replace(/\/$/, "")}/chat/completions`;

  return {
    type: "configure_ai_provider",
    provider: {
      id: `codex-${config.providerName}`,
      curl: [
        `curl ${endpoint}`,
        '-H "Content-Type: application/json"',
        '-H "Authorization: Bearer {{API_KEY}}"',
        '-H "api-key: {{API_KEY}}"',
        `-d '${JSON.stringify({
          model: "{{MODEL}}",
          messages: [
            { role: "system", content: "{{SYSTEM_PROMPT}}" },
            { role: "user", content: "{{TEXT}}" },
          ],
          stream: false,
        })}'`,
      ].join(" \\\n  "),
      streaming: false,
      responseContentPath: "choices[0].message.content",
    },
    variables: { api_key: config.apiKey, model: config.model },
    select: true,
  };
}

function createCodexSpeechCommand() {
  const config = readCodexProviderConfig();
  const model = process.env.PLUELY_CODEX_SPEECH_MODEL || "gpt-transcribe";
  const endpoint = `${config.baseUrl.replace(/\/$/, "")}/audio/transcriptions`;

  return {
    type: "configure_stt_provider",
    provider: {
      id: `codex-${config.providerName}-speech`,
      curl: [
        `curl -X POST "${endpoint}"`,
        '-H "Authorization: Bearer {{API_KEY}}"',
        '-H "api-key: {{API_KEY}}"',
        '-F "file={{AUDIO}}"',
        '-F "model={{MODEL}}"',
      ].join(" \\\n  "),
      streaming: false,
      responseContentPath: "text",
    },
    variables: { api_key: config.apiKey, model },
    select: true,
  };
}

function createLocalSpeechCommand(token) {
  return {
    type: "configure_stt_provider",
    provider: {
      id: "local-whisper",
      curl: [
        `curl -X POST "${address}/v1/speech/transcriptions"`,
        '-H "Authorization: Bearer {{API_KEY}}"',
        '-H "Content-Type: audio/wav"',
        '--data-binary {{AUDIO}}',
      ].join(" \\\n  "),
      streaming: false,
      responseContentPath: "text",
    },
    variables: { api_key: token },
    select: true,
  };
}

function readCodexProviderConfig() {
  const configPath =
    process.env.CODEX_CONFIG_PATH || path.join(os.homedir(), ".codex", "config.toml");
  const source = fs.readFileSync(configPath, "utf8");
  const model = readTopLevelValue(source, "model");
  const providerName = readTopLevelValue(source, "model_provider");
  if (!model || !providerName) fail("Codex config is missing model or model_provider.");

  const section = readTomlSection(source, `model_providers.${providerName}`);
  const baseUrl = readValue(section, "base_url");
  const envKey = readValue(section, "env_key");
  if (!baseUrl || !envKey) fail(`Codex provider ${providerName} is missing base_url or env_key.`);

  const apiKey = process.env[envKey];
  if (!apiKey) fail(`The ${envKey} environment variable is not available.`);
  return {
    model,
    providerName,
    baseUrl,
    apiKey,
  };
}

function resolveTokenPath() {
  if (process.env.PLUELY_CLEAN_TOKEN_PATH) return process.env.PLUELY_CLEAN_TOKEN_PATH;
  if (process.platform === "darwin") {
    return path.join(
      os.homedir(),
      "Library",
      "Application Support",
      "com.joshua.pluely-clean",
      "control-token"
    );
  }
  if (process.platform === "win32") {
    return path.join(
      process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming"),
      "com.joshua.pluely-clean",
      "control-token"
    );
  }
  return path.join(
    process.env.XDG_DATA_HOME || path.join(os.homedir(), ".local", "share"),
    "com.joshua.pluely-clean",
    "control-token"
  );
}

function readTomlSection(source, sectionName) {
  const lines = source.split(/\r?\n/);
  const heading = `[${sectionName}]`;
  const start = lines.findIndex((line) => line.trim() === heading);
  if (start === -1) return "";

  const section = [];
  for (const line of lines.slice(start + 1)) {
    if (line.trim().startsWith("[")) break;
    section.push(line);
  }
  return section.join("\n");
}

function readTopLevelValue(source, key) {
  return readValue(source.split(/^\[/m, 1)[0], key);
}

function readValue(source, key) {
  const match = source.match(new RegExp(`^${key}\\s*=\\s*["']([^"']+)["']\\s*$`, "m"));
  return match?.[1];
}

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}
