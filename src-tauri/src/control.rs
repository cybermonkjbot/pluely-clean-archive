use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io, path::PathBuf, time::Duration};
use std::process::Stdio;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;
use uuid::Uuid;

const CONTROL_ADDRESS: &str = "127.0.0.1:37421";
const MAX_REQUEST_BYTES: usize = 20 * 1_048_576;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: String,
    pub curl: String,
    pub streaming: bool,
    pub response_content_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlCommand {
    ConfigureAiProvider {
        provider: ProviderConfig,
        #[serde(default)]
        variables: HashMap<String, String>,
        #[serde(default = "default_true")]
        select: bool,
    },
    ConfigureSttProvider {
        provider: ProviderConfig,
        #[serde(default)]
        variables: HashMap<String, String>,
        #[serde(default = "default_true")]
        select: bool,
    },
    SetSystemPrompt { prompt: String },
    SetInterviewContext {
        context: Option<crate::interview::InterviewContext>,
    },
    ReloadSettings,
    OpenDashboard,
    ShowOverlay,
    HideOverlay,
    ToggleOverlay,
}

fn default_true() -> bool {
    true
}

pub fn start(app: AppHandle) -> Result<(), String> {
    let token = load_or_create_token(&app).map_err(|error| error.to_string())?;

    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::bind(CONTROL_ADDRESS).await {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("Pluely Clean control layer could not bind: {error}");
                return;
            }
        };

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let app = app.clone();
                    let token = token.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = handle_connection(stream, app, token).await {
                            eprintln!("Pluely Clean control request failed: {error}");
                        }
                    });
                }
                Err(error) => eprintln!("Pluely Clean control accept failed: {error}"),
            }
        }
    });

    Ok(())
}

fn load_or_create_token(app: &AppHandle) -> io::Result<String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
    fs::create_dir_all(&directory)?;
    let path: PathBuf = directory.join("control-token");

    if let Ok(existing) = fs::read_to_string(&path) {
        let token = existing.trim();
        if !token.is_empty() {
            return Ok(token.to_string());
        }
    }

    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    fs::write(&path, format!("{token}\n"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }

    Ok(token)
}

async fn handle_connection(
    mut stream: TcpStream,
    app: AppHandle,
    token: String,
) -> Result<(), String> {
    let request = read_request(&mut stream).await?;

    if request.method == "GET" && request.path == "/v1/health" {
        return write_json(&mut stream, 200, r#"{"ok":true}"#).await;
    }

    if request.method == "OPTIONS" {
        return write_preflight(&mut stream).await;
    }

    let expected = format!("Bearer {token}");
    if request.headers.get("authorization") != Some(&expected) {
        return write_json(&mut stream, 401, r#"{"error":"unauthorized"}"#).await;
    }

    if request.method == "POST" && request.path == "/v1/speech/transcriptions" {
        return handle_local_transcription(&mut stream, &app, &request.body).await;
    }

    if request.method == "POST" && request.path == "/v1/codex/chat/completions" {
        return handle_codex_chat(&mut stream, &app, &request.body).await;
    }

    if request.method == "GET" && request.path == "/v1/interview-context" {
        let context = crate::interview::get_calendar_interview_context().await?;
        let body = serde_json::to_string(&context)
            .map_err(|error| format!("Failed to serialize interview context: {error}"))?;
        return write_json(&mut stream, 200, &body).await;
    }

    if request.method != "POST" || request.path != "/v1/commands" {
        return write_json(&mut stream, 404, r#"{"error":"not_found"}"#).await;
    }

    let command: ControlCommand = serde_json::from_slice(&request.body)
        .map_err(|error| format!("Invalid control command: {error}"))?;
    apply_command(&app, &command)?;
    write_json(&mut stream, 202, r#"{"accepted":true}"#).await
}

async fn handle_local_transcription(
    stream: &mut TcpStream,
    app: &AppHandle,
    audio: &[u8],
) -> Result<(), String> {
    if audio.is_empty() {
        return write_json(stream, 400, r#"{"error":"empty_audio"}"#).await;
    }

    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Failed to resolve app data directory: {error}"))?;
    let speech_dir = app_data.join("speech");
    let binary = speech_dir.join("whisper-cli");
    let model = speech_dir.join("ggml-base.en.bin");
    if !binary.is_file() || !model.is_file() {
        return write_json(stream, 503, r#"{"error":"local_speech_not_installed"}"#).await;
    }

    let request_id = Uuid::new_v4().simple().to_string();
    let input = speech_dir.join(format!("{request_id}.wav"));
    let output_prefix = speech_dir.join(&request_id);
    let output_text = speech_dir.join(format!("{request_id}.txt"));
    fs::write(&input, audio).map_err(|error| format!("Failed to stage audio: {error}"))?;

    let result = tokio::time::timeout(
        Duration::from_secs(120),
        Command::new(&binary)
            .env("DYLD_LIBRARY_PATH", &speech_dir)
            .arg("-m")
            .arg(&model)
            .arg("-f")
            .arg(&input)
            .arg("-otxt")
            .arg("-of")
            .arg(&output_prefix)
            .arg("-np")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output(),
    )
    .await;

    let response = match result {
        Err(_) => serde_json::json!({ "error": "transcription_timeout" }),
        Ok(Err(error)) => serde_json::json!({ "error": format!("Failed to start local speech: {error}") }),
        Ok(Ok(output)) if !output.status.success() => serde_json::json!({
            "error": String::from_utf8_lossy(&output.stderr).trim()
        }),
        Ok(Ok(_)) => match fs::read_to_string(&output_text) {
            Ok(text) => serde_json::json!({ "text": text.trim() }),
            Err(error) => serde_json::json!({ "error": format!("Failed to read transcript: {error}") }),
        },
    };

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output_text);
    let status = if response.get("text").is_some() { 200 } else { 502 };
    write_json(stream, status, &response.to_string()).await
}

#[derive(Deserialize)]
struct CodexChatRequest {
    model: String,
    messages: Vec<CodexChatMessage>,
}

#[derive(Deserialize)]
struct CodexChatMessage {
    role: String,
    content: serde_json::Value,
}

async fn handle_codex_chat(
    stream: &mut TcpStream,
    app: &AppHandle,
    body: &[u8],
) -> Result<(), String> {
    let request: CodexChatRequest = serde_json::from_slice(body)
        .map_err(|error| format!("Invalid Codex chat request: {error}"))?;
    let prompt = request
        .messages
        .iter()
        .map(|message| format!("{}:\n{}", message.role, message_content(&message.content)))
        .collect::<Vec<_>>()
        .join("\n\n");
    let working_directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Failed to resolve app data directory: {error}"))?;
    fs::create_dir_all(&working_directory)
        .map_err(|error| format!("Failed to prepare Codex working directory: {error}"))?;

    let mut child = Command::new(resolve_codex_binary())
        .arg("exec")
        .arg("--ephemeral")
        .arg("--sandbox")
        .arg("read-only")
        .arg("--skip-git-repo-check")
        .arg("--color")
        .arg("never")
        .arg("--model")
        .arg(&request.model)
        .arg("-")
        .current_dir(working_directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to start Codex CLI: {error}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .await
            .map_err(|error| format!("Failed to send prompt to Codex CLI: {error}"))?;
    }

    let output = tokio::time::timeout(Duration::from_secs(180), child.wait_with_output())
        .await
        .map_err(|_| "Codex CLI timed out after 180 seconds".to_string())?
        .map_err(|error| format!("Codex CLI failed: {error}"))?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let body = serde_json::json!({ "error": error }).to_string();
        return write_json(stream, 502, &body).await;
    }

    let content = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let body = serde_json::json!({
        "id": format!("codex-{}", Uuid::new_v4()),
        "object": "chat.completion",
        "model": request.model,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": "stop"
        }]
    })
    .to_string();
    write_json(stream, 200, &body).await
}

fn resolve_codex_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("PLUELY_CODEX_BIN") {
        return PathBuf::from(path);
    }

    let mut candidates = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(home).join(".bun/bin/codex"));
    }
    candidates.extend([
        PathBuf::from("/opt/homebrew/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
    ]);
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("codex"))
}

fn message_content(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(value) => value.clone(),
        value => value.to_string(),
    }
}

fn apply_command(app: &AppHandle, command: &ControlCommand) -> Result<(), String> {
    match command {
        ControlCommand::OpenDashboard => crate::window::show_dashboard_window(app),
        ControlCommand::ShowOverlay => set_overlay_visibility(app, Some(true)),
        ControlCommand::HideOverlay => set_overlay_visibility(app, Some(false)),
        ControlCommand::ToggleOverlay => set_overlay_visibility(app, None),
        _ => app
            .emit("pluely-control-command", command)
            .map_err(|error| format!("Failed to emit control command: {error}")),
    }
}

fn set_overlay_visibility(app: &AppHandle, visible: Option<bool>) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main overlay window not found".to_string())?;
    let should_show = match visible {
        Some(value) => value,
        None => !window
            .is_visible()
            .map_err(|error| format!("Failed to read overlay visibility: {error}"))?,
    };

    if should_show {
        window
            .show()
            .map_err(|error| format!("Failed to show overlay: {error}"))?;
    } else {
        window
            .hide()
            .map_err(|error| format!("Failed to hide overlay: {error}"))?;
    }
    Ok(())
}

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

async fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    let header_end;

    loop {
        let count = stream
            .read(&mut buffer)
            .await
            .map_err(|error| format!("Failed to read request: {error}"))?;
        if count == 0 {
            return Err("Connection closed before request completed".to_string());
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err("Control request exceeded size limit".to_string());
        }
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            header_end = position + 4;
            break;
        }
    }

    let header_text = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| "Control request headers were not UTF-8".to_string())?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "Missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_string();
    let path = request_parts.next().unwrap_or_default().to_string();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect::<HashMap<_, _>>();
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);

    while bytes.len() < header_end + content_length {
        let count = stream
            .read(&mut buffer)
            .await
            .map_err(|error| format!("Failed to read request body: {error}"))?;
        if count == 0 {
            return Err("Connection closed before request body completed".to_string());
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err("Control request exceeded size limit".to_string());
        }
    }

    Ok(HttpRequest {
        method,
        path,
        headers,
        body: bytes[header_end..header_end + content_length].to_vec(),
    })
}

async fn write_json(stream: &mut TcpStream, status: u16, body: &str) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        401 => "Unauthorized",
        400 => "Bad Request",
        404 => "Not Found",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|error| format!("Failed to write response: {error}"))
}

async fn write_preflight(stream: &mut TcpStream) -> Result<(), String> {
    let response = concat!(
        "HTTP/1.1 204 No Content\r\n",
        "Access-Control-Allow-Origin: *\r\n",
        "Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n",
        "Access-Control-Allow-Headers: Authorization, Content-Type\r\n",
        "Access-Control-Max-Age: 86400\r\n",
        "Content-Length: 0\r\n",
        "Connection: close\r\n\r\n"
    );
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|error| format!("Failed to write preflight response: {error}"))
}
