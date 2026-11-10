use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}, time::Duration};
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
    #[serde(default)]
    pub resume_path: Option<String>,
    #[serde(default = "default_resume_match")]
    pub resume_match: String,
}

fn default_resume_match() -> String {
    "unspecified".to_string()
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

    let interviewing_as = metadata_value(&metadata, &["as", "interviewing as", "role"])
        .or_else(|| {
            metadata_value(&metadata, &["jd", "job description"])
                .and_then(|value| role_from_job_url(&value))
        })
        .or_else(|| role_from_title(&event.title));

    let resume_version = metadata_value(&metadata, &["resume", "resume version", "cv"]);
    let (resume_path, resume_match) = match_resume(
        resume_version.as_deref(),
        interviewing_as.as_deref(),
    );
    let attendees = if event.attendees.is_empty() {
        attendees_from_title(&event.title)
    } else {
        event.attendees
    };

    Some(InterviewContext {
        event_id: event.id,
        title: event.title,
        starts_at: event.starts_at,
        ends_at: event.ends_at,
        location: optional_text(event.location),
        attendees,
        interviewing_as,
        target_time_zone: metadata_value(&metadata, &["timezone", "time zone", "target timezone"]),
        resume_version,
        resume_path,
        resume_match,
    })
}

fn match_resume(resume_version: Option<&str>, interviewing_as: Option<&str>) -> (Option<String>, String) {
    if let Some(value) = resume_version {
        if let Some(path) = resolve_explicit_path(value) {
            return (Some(path.to_string_lossy().into_owned()), "matched".to_string());
        }
    }

    let candidates = resume_candidates();
    let query = resume_version.or(interviewing_as);
    let Some(query) = query else {
        return (None, "unspecified".to_string());
    };
    let query = normalized(query);
    if query.is_empty() {
        return (None, "unspecified".to_string());
    }

    let mut ranked = candidates
        .into_iter()
        .filter_map(|path| {
            let name = normalized(path.file_stem()?.to_str()?);
            let score = resume_score(&query, &name);
            (score > 0).then_some((score, path))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.0.cmp(&left.0));

    let Some((best_score, best_path)) = ranked.first() else {
        return (None, "missing".to_string());
    };
    if *best_score < 60 {
        return (None, "missing".to_string());
    }
    if ranked.get(1).is_some_and(|(score, _)| score == best_score) {
        return (None, "ambiguous".to_string());
    }
    (
        Some(best_path.to_string_lossy().into_owned()),
        "matched".to_string(),
    )
}

fn resolve_explicit_path(value: &str) -> Option<PathBuf> {
    if value.starts_with("https://") || value.starts_with("http://") {
        return Some(PathBuf::from(value));
    }
    let expanded = if let Some(relative) = value.strip_prefix("~/") {
        std::env::var_os("HOME").map(PathBuf::from)?.join(relative)
    } else {
        PathBuf::from(value)
    };
    expanded.is_file().then_some(expanded)
}

fn resume_candidates() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for directory in ["Documents", "Desktop", "Downloads"] {
        collect_resume_files(&home.join(directory), 0, &mut candidates);
    }
    candidates
}

fn collect_resume_files(directory: &Path, depth: usize, candidates: &mut Vec<PathBuf>) {
    if depth > 4 || candidates.len() >= 200 {
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_resume_files(&path, depth + 1, candidates);
            continue;
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !["pdf", "doc", "docx", "txt", "md"].contains(&extension.as_str()) {
            continue;
        }
        let name = normalized(path.file_name().and_then(|value| value.to_str()).unwrap_or_default());
        if name.contains("resume") || name.contains("résumé") || name.split_whitespace().any(|part| part == "cv") {
            candidates.push(path);
        }
    }
}

fn resume_score(query: &str, candidate: &str) -> u16 {
    if query == candidate {
        return 100;
    }
    if candidate.contains(query) || query.contains(candidate) {
        return 90;
    }
    let tokens = query
        .split_whitespace()
        .filter(|token| token.len() > 1 && !["resume", "résumé", "cv", "version"].contains(token))
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return 0;
    }
    let matches = tokens.iter().filter(|token| candidate.contains(**token)).count();
    ((matches * 80) / tokens.len()) as u16
}

fn normalized(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

fn role_from_job_url(value: &str) -> Option<String> {
    let slug = value
        .split('?')
        .next()?
        .trim_end_matches('/')
        .rsplit('/')
        .nth(1)?;
    let role = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    (!role.is_empty()).then_some(role)
}

fn attendees_from_title(title: &str) -> Vec<String> {
    title
        .split('@')
        .next()
        .unwrap_or_default()
        .split('-')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn contains_interview_term(value: &str) -> bool {
    let value = value.to_lowercase();
    ["interview", "recruiter screen", "technical screen", "hiring manager"]
        .iter()
        .any(|term| value.contains(term))
}

fn parse_metadata(notes: &str) -> Vec<(String, String)> {
    let lines = notes.lines().map(str::trim).collect::<Vec<_>>();
    let known_keys = [
        "as",
        "interviewing as",
        "role",
        "timezone",
        "time zone",
        "target timezone",
        "resume",
        "resume version",
        "cv",
        "jd",
        "job description",
        "salary",
    ];
    let mut metadata = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_lowercase();
            let value = value.trim();
            if known_keys.contains(&key.as_str()) && !value.is_empty() {
                metadata.push((key, value.to_string()));
            }
            continue;
        }

        let key = line.to_lowercase();
        if !known_keys.contains(&key.as_str()) {
            continue;
        }
        if let Some(value) = lines[index + 1..].iter().find(|value| !value.is_empty()) {
            metadata.push((key, (*value).to_string()));
        }
    }
    metadata
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
