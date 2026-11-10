import { AlertTriangleIcon, CalendarDaysIcon, CheckCircle2Icon } from "lucide-react";
import type { InterviewContext } from "@/lib";

const resumeLabel = (context: InterviewContext) => {
  if (context.resumePath) {
    if (context.resumePath.startsWith("http")) return "Calendar résumé link";
    return context.resumePath.split("/").pop() || context.resumePath;
  }
  return context.resumeVersion || "Not specified";
};

const scheduledTime = (value: string) => {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    weekday: "short",
    hour: "numeric",
    minute: "2-digit",
    timeZoneName: "short",
  }).format(parsed);
};

export const InterviewReadiness = ({
  context,
}: {
  context: InterviewContext;
}) => {
  const missing = [
    !context.interviewingAs && "role",
    !context.targetTimeZone && "target time zone",
    context.resumeMatch !== "matched" && "résumé",
  ].filter(Boolean) as string[];
  const ready = missing.length === 0;

  return (
    <section className="rounded-lg border border-border/50 bg-muted/30 p-2.5 space-y-2">
      <div className="flex items-start gap-2">
        {ready ? (
          <CheckCircle2Icon className="mt-0.5 h-3.5 w-3.5 shrink-0 text-green-600" />
        ) : (
          <AlertTriangleIcon className="mt-0.5 h-3.5 w-3.5 shrink-0 text-amber-600" />
        )}
        <div className="min-w-0 flex-1">
          <p className="truncate text-xs font-medium">{context.title}</p>
          <p className="text-[10px] text-muted-foreground">
            {ready ? "Interview context ready" : `Missing ${missing.join(", ")}`}
          </p>
        </div>
        <div className="flex items-center gap-1 text-[10px] text-muted-foreground">
          <CalendarDaysIcon className="h-3 w-3" />
          {scheduledTime(context.startsAt)}
        </div>
      </div>

      <dl className="grid grid-cols-[5rem_1fr] gap-x-2 gap-y-1 text-[10px]">
        <dt className="text-muted-foreground">Interviewing as</dt>
        <dd className="truncate">{context.interviewingAs || "Not specified"}</dd>
        <dt className="text-muted-foreground">Time zone</dt>
        <dd className="truncate">{context.targetTimeZone || "Not specified"}</dd>
        <dt className="text-muted-foreground">Résumé</dt>
        <dd className="truncate">{resumeLabel(context)}</dd>
        {context.attendees.length > 0 && (
          <>
            <dt className="text-muted-foreground">Participants</dt>
            <dd className="truncate">{context.attendees.join(", ")}</dd>
          </>
        )}
      </dl>
    </section>
  );
};
