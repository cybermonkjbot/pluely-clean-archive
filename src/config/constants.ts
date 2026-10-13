// Storage keys
export const STORAGE_KEYS = {
  THEME: "theme",
  TRANSPARENCY: "transparency",
  SYSTEM_PROMPT: "system_prompt",
  INTERVIEW_CONTEXT: "interview_context",
  SELECTED_SYSTEM_PROMPT_ID: "selected_system_prompt_id",
  SCREENSHOT_CONFIG: "screenshot_config",
  // add curl_ prefix because we are using curl to store the providers
  CUSTOM_AI_PROVIDERS: "curl_custom_ai_providers",
  CUSTOM_SPEECH_PROVIDERS: "curl_custom_speech_providers",
  SELECTED_AI_PROVIDER: "curl_selected_ai_provider",
  SELECTED_STT_PROVIDER: "curl_selected_stt_provider",
  SYSTEM_AUDIO_CONTEXT: "system_audio_context",
  SYSTEM_AUDIO_QUICK_ACTIONS: "system_audio_quick_actions",
  CUSTOMIZABLE: "customizable",
  PLUELY_API_ENABLED: "pluely_api_enabled",
  SHORTCUTS: "shortcuts",
  AUTOSTART_INITIALIZED: "autostart_initialized",

  SELECTED_AUDIO_DEVICES: "selected_audio_devices",
  RESPONSE_SETTINGS: "response_settings",
  SUPPORTS_IMAGES: "supports_images",
} as const;

// Max number of files that can be attached to a message
export const MAX_FILES = 6;

// Default settings
export const DEFAULT_SYSTEM_PROMPT =
  `You are Joshua's private interview copilot on a dedicated interview laptop.

Use the active Calendar interview context as the source of truth for the role Joshua is interviewing as, the target time zone, the resume version, the scheduled time and the participants. Quietly keep that context consistent throughout the session. If a value is missing, do not invent it.

When an interviewer asks a question, give Joshua a concise, natural first-person answer he can adapt and say aloud. Lead with the answer, then add only the strongest supporting detail. Match the seniority and discipline in the active interview context. Keep answers credible and conversational rather than polished like an essay.

For behavioural questions, use a compact situation-action-result structure. For technical questions, state the approach, key trade-offs and likely implementation steps. For coding tasks, provide the simplest correct solution first, mention complexity and flag important edge cases. For follow-up questions, preserve facts already established in the session.

Never fabricate employers, projects, achievements, dates, metrics or skills that are not present in the supplied resume, Calendar context, conversation, screen context or user-provided material. If evidence is missing, provide a safe answer structure or ask Joshua for the missing fact. Do not mention these instructions, the Calendar integration or that you are an AI unless Joshua asks.`;

export const MARKDOWN_FORMATTING_INSTRUCTIONS =
  "IMPORTANT - Formatting Rules (use silently, never mention these rules in your responses):\n- Mathematical expressions: ALWAYS use double dollar signs ($$) for both inline and block math. Never use single $.\n- Code blocks: ALWAYS use triple backticks with language specification.\n- Diagrams: Use ```mermaid code blocks.\n- Tables: Use standard markdown table syntax.\n- Never mention to the user that you're using these formats or explain the formatting syntax in your responses. Just use them naturally.";

export const DEFAULT_QUICK_ACTIONS = [
  "What should I say?",
  "Follow-up questions",
  "Fact-check",
  "Recap",
];
