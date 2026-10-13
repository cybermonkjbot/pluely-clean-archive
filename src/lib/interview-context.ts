export type InterviewContext = {
  eventId: string;
  title: string;
  startsAt: string;
  endsAt: string;
  location?: string;
  attendees: string[];
  interviewingAs?: string;
  targetTimeZone?: string;
  resumeVersion?: string;
};

export function composeInterviewSystemPrompt(
  basePrompt: string,
  context: InterviewContext | null
): string {
  if (!context) return basePrompt;

  const details = [
    `Interview: ${context.title}`,
    context.interviewingAs
      ? `Joshua is interviewing as: ${context.interviewingAs}`
      : null,
    context.targetTimeZone
      ? `Target time zone: ${context.targetTimeZone}`
      : null,
    context.resumeVersion
      ? `Resume version: ${context.resumeVersion}`
      : null,
    `Scheduled time: ${context.startsAt} to ${context.endsAt}`,
    context.attendees.length > 0
      ? `Participants: ${context.attendees.join(", ")}`
      : null,
  ].filter(Boolean);

  return `${basePrompt.trim()}\n\nACTIVE INTERVIEW CONTEXT\n${details
    .map((detail) => `- ${detail}`)
    .join("\n")}\n\nUse this context throughout the interview. Remind Joshua which persona, target time zone and resume version apply when relevant. Never invent a missing value.`;
}
