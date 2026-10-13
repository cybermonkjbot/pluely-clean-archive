import { STORAGE_KEYS } from "@/config";
import { safeLocalStorage } from "@/lib/storage";
import type { TYPE_PROVIDER } from "@/types";
import type { InterviewContext } from "./interview-context";

type ProviderCommand = {
  provider: TYPE_PROVIDER & { id: string };
  variables: Record<string, string>;
  select: boolean;
};

export type ControlCommand =
  | ({ type: "configure_ai_provider" } & ProviderCommand)
  | ({ type: "configure_stt_provider" } & ProviderCommand)
  | { type: "set_system_prompt"; prompt: string }
  | { type: "set_interview_context"; context: InterviewContext | null }
  | { type: "reload_settings" };

export function applyControlCommand(command: ControlCommand): void {
  switch (command.type) {
    case "configure_ai_provider":
      storeProvider(
        STORAGE_KEYS.CUSTOM_AI_PROVIDERS,
        STORAGE_KEYS.SELECTED_AI_PROVIDER,
        command
      );
      return;
    case "configure_stt_provider":
      storeProvider(
        STORAGE_KEYS.CUSTOM_SPEECH_PROVIDERS,
        STORAGE_KEYS.SELECTED_STT_PROVIDER,
        command
      );
      return;
    case "set_system_prompt":
      safeLocalStorage.setItem(STORAGE_KEYS.SYSTEM_PROMPT, command.prompt);
      return;
    case "set_interview_context":
      if (command.context) {
        safeLocalStorage.setItem(
          STORAGE_KEYS.INTERVIEW_CONTEXT,
          JSON.stringify(command.context)
        );
      } else {
        safeLocalStorage.removeItem(STORAGE_KEYS.INTERVIEW_CONTEXT);
      }
      return;
    case "reload_settings":
      return;
  }
}

function storeProvider(
  providersKey: string,
  selectedKey: string,
  command: ProviderCommand
): void {
  const existing = readProviders(providersKey);
  const provider = { ...command.provider, isCustom: true };
  const next = [
    ...existing.filter((item) => item.id !== provider.id),
    provider,
  ];
  safeLocalStorage.setItem(providersKey, JSON.stringify(next));

  if (command.select) {
    safeLocalStorage.setItem(
      selectedKey,
      JSON.stringify({
        provider: provider.id,
        variables: normalizeVariables(command.variables),
      })
    );
  }
}

function readProviders(key: string): TYPE_PROVIDER[] {
  try {
    const parsed = JSON.parse(safeLocalStorage.getItem(key) || "[]");
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function normalizeVariables(
  variables: Record<string, string>
): Record<string, string> {
  return Object.fromEntries(
    Object.entries(variables).map(([key, value]) => [
      key.toLowerCase(),
      value,
    ])
  );
}
