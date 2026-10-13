use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::process::Command;

const FIELD_SEPARATOR: char = '\u{001f}';
const RECORD_SEPARATOR: char = '\u{001e}';

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewContext {
    pub event_id: String,
    pub title: String,
    pub starts_at: String,
    pub ends_at: String,
    pub location: Option<String>,
    pub attendees: Vec<String>,
    pub interviewing_as: Option<String>,
    pub target_time_zone: Option<String>,
    pub resume_version: Option<String>,
}

#[derive(Debug)]
struct CalendarEvent {
    calendar_name: String,
    id: String,
    title: String,
    notes: String,
    starts_at: String,
    ends_at: String,
    location: String,
    attendees: Vec<String>,
    seconds_until_start: f64,
}

#[tauri::command]
pub async fn get_calendar_interview_context() -> Result<Option<InterviewContext>, String> {
    #[cfg(not(target_os = "macos"))]
    return Ok(None);

    #[cfg(target_os = "macos")]
    {
        let output = tokio::time::timeout(
            Duration::from_secs(15),
            Command::new("/usr/bin/osascript")
                .arg("-e")
                .arg(calendar_script())
                .output(),
        )
        .await
        .map_err(|_| "macOS Calendar lookup timed out".to_string())?
        .map_err(|error| format!("Failed to read macOS Calendar: {error}"))?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }

        let mut events = parse_events(&String::from_utf8_lossy(&output.stdout));
        events.sort_by(|left, right| {
            left.seconds_until_start
                .partial_cmp(&right.seconds_until_start)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(events.into_iter().find_map(to_interview_context))
    }
}

fn to_interview_context(event: CalendarEvent) -> Option<InterviewContext> {
    let metadata = parse_metadata(&event.notes);
    let explicit_marker = event.notes.to_lowercase().contains("[interview]")
        || contains_interview_term(&event.calendar_name)
        || metadata.iter().any(|(key, _)| {
            ["as", "interviewing as", "role", "resume", "resume version", "cv"]
                .contains(&key.as_str())
        });
    let title_matches = contains_interview_term(&event.title);
    if !explicit_marker && !title_matches {
        return None;
    }

    Some(InterviewContext {
        event_id: event.id,
        title: event.title,
        starts_at: event.starts_at,
        ends_at: event.ends_at,
        location: optional_text(event.location),
        attendees: event.attendees,
        interviewing_as: metadata_value(&metadata, &["as", "interviewing as", "role"])
            .or_else(|| role_from_title(&event.title)),
        target_time_zone: metadata_value(&metadata, &["timezone", "time zone", "target timezone"]),
        resume_version: metadata_value(&metadata, &["resume", "resume version", "cv"]),
    })
}

fn role_from_title(title: &str) -> Option<String> {
    let lower = title.to_lowercase();
    for marker in ["interview as ", "interview: ", "interview - "] {
        if let Some(position) = lower.find(marker) {
            let value = title[position + marker.len()..].trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn contains_interview_term(value: &str) -> bool {
    let value = value.to_lowercase();
    ["interview", "recruiter screen", "technical screen", "hiring manager"]
        .iter()
        .any(|term| value.contains(term))
}

fn parse_metadata(notes: &str) -> Vec<(String, String)> {
    notes
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_lowercase(), value.trim().to_string()))
        .filter(|(_, value)| !value.is_empty())
        .collect()
}

fn metadata_value(metadata: &[(String, String)], keys: &[&str]) -> Option<String> {
    metadata
        .iter()
        .find(|(key, _)| keys.iter().any(|candidate| key == candidate))
        .map(|(_, value)| value.clone())
}

fn optional_text(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn parse_events(output: &str) -> Vec<CalendarEvent> {
    output
        .trim_matches(|character| character == '\n' || character == '\r')
        .split(RECORD_SEPARATOR)
        .filter_map(|record| {
            let fields = record.split(FIELD_SEPARATOR).collect::<Vec<_>>();
            if fields.len() != 9 {
                return None;
            }
            Some(CalendarEvent {
                calendar_name: fields[0].trim().to_string(),
                id: fields[1].trim().to_string(),
                title: fields[2].trim().to_string(),
                notes: fields[3].trim().to_string(),
                starts_at: fields[4].trim().to_string(),
                ends_at: fields[5].trim().to_string(),
                location: fields[6].trim().to_string(),
                attendees: fields[7]
                    .split(";")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect(),
                seconds_until_start: fields[8].trim().parse().unwrap_or(f64::MAX),
            })
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn calendar_script() -> &'static str {
    r#"
set fieldSeparator to ASCII character 31
set recordSeparator to ASCII character 30
set startWindow to current date
set endWindow to (current date) + (24 * hours)
set resultText to ""

on cleanValue(rawValue, fieldSeparator, recordSeparator)
    if rawValue is missing value then return ""
    set valueText to rawValue as text
    set AppleScript's text item delimiters to fieldSeparator
    set valueParts to text items of valueText
    set AppleScript's text item delimiters to " "
    set valueText to valueParts as text
    set AppleScript's text item delimiters to recordSeparator
    set valueParts to text items of valueText
    set AppleScript's text item delimiters to " "
    set valueText to valueParts as text
    set AppleScript's text item delimiters to ""
    return valueText
end cleanValue

tell application "Calendar"
    repeat with calendarItem in calendars
        set matchingEvents to every event of calendarItem whose start date is less than or equal to endWindow and end date is greater than or equal to startWindow and allday event is false
        repeat with eventItem in matchingEvents
            set attendeeText to ""
            repeat with attendeeItem in attendees of eventItem
                try
                    set attendeeName to display name of attendeeItem
                    if attendeeName is missing value or attendeeName is "" then set attendeeName to email of attendeeItem
                    if attendeeText is not "" then set attendeeText to attendeeText & "; "
                    set attendeeText to attendeeText & attendeeName
                end try
            end repeat
            set secondsUntilStart to (start date of eventItem) - (current date)
            set eventRecord to my cleanValue(name of calendarItem, fieldSeparator, recordSeparator) & fieldSeparator & my cleanValue(uid of eventItem, fieldSeparator, recordSeparator) & fieldSeparator & my cleanValue(summary of eventItem, fieldSeparator, recordSeparator) & fieldSeparator & my cleanValue(description of eventItem, fieldSeparator, recordSeparator) & fieldSeparator & my cleanValue(start date of eventItem, fieldSeparator, recordSeparator) & fieldSeparator & my cleanValue(end date of eventItem, fieldSeparator, recordSeparator) & fieldSeparator & my cleanValue(location of eventItem, fieldSeparator, recordSeparator) & fieldSeparator & my cleanValue(attendeeText, fieldSeparator, recordSeparator) & fieldSeparator & secondsUntilStart
            if resultText is not "" then set resultText to resultText & recordSeparator
            set resultText to resultText & eventRecord
        end repeat
    end repeat
end tell

return resultText
"#
}
