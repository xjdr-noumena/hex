//! Delegations answered by an OpenCode session, through the router crate's client (it finds the
//! local OpenCode server and its password the same way Annotate does).

use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use scrawl_router::opencode;
use serde_json::Value;

const POLL: Duration = Duration::from_millis(400);
const TIMEOUT: Duration = Duration::from_secs(600);
/// The most reply text handed to the voice model; it paraphrases, it does not read code aloud.
pub const MAX_SPOKEN_CHARS: usize = 2_000;

fn get(path: &str) -> Result<Value> {
    let body = opencode::api_within("GET", path, None, Duration::from_secs(5))?;
    let value: Value = serde_json::from_slice(&body).context("invalid OpenCode response")?;
    Ok(value.get("data").cloned().unwrap_or(value))
}

/// The newest open root session whose working directory still exists on its server.
pub fn latest_session() -> Result<String> {
    resolve_session(None)
}

/// Check the destination before starting media, using read-only requests on its server.
pub fn resolve_session(explicit: Option<&str>) -> Result<String> {
    let mut sessions = |path: &str| opencode::api_get_within(path, Duration::from_secs(5));
    let mut directories = |path: &str| opencode::api_get_within(path, Duration::from_secs(5));
    resolve_session_with(explicit, &mut sessions, &mut directories)
}

fn directory_available(
    session: &opencode::OpenCodeSession,
    get: &mut opencode::ApiGet<'_>,
) -> Result<bool> {
    let Some(directory) = session.directory() else {
        return Ok(false);
    };
    // Check on the OpenCode server, which may be on another host. Metadata and
    // message reads can still succeed for a session whose directory was deleted.
    let directory = opencode::query_escape(directory);
    let path = format!("/api/fs/list?location[directory]={directory}&path=.");
    match get(&path) {
        Ok(_) => Ok(true),
        Err(error) if opencode::http_status(&error) == Some(404) => Ok(false),
        Err(error) => Err(error).context("could not check the OpenCode session directory"),
    }
}

fn resolve_session_with(
    explicit: Option<&str>,
    sessions: &mut opencode::ApiGet<'_>,
    directories: &mut opencode::ApiGet<'_>,
) -> Result<String> {
    if let Some(id) = explicit {
        let session = opencode::get_session(sessions, id)?;
        if !directory_available(&session, directories)? {
            bail!(
                "OpenCode session {id} has no existing working directory; choose another session"
            );
        }
        return Ok(session.id);
    }
    let mut failure = None;
    let session =
        opencode::find_root_session(sessions, "", opencode::ROOT_SEARCH_PAGES, |session| {
            match directory_available(session, directories) {
                Ok(true) => opencode::Scan::Take,
                Ok(false) => opencode::Scan::Skip,
                Err(error) => {
                    failure = Some(error);
                    opencode::Scan::Stop
                }
            }
        })?;
    if let Some(error) = failure {
        return Err(error);
    }
    session
        .map(|session| session.id)
        .context("no OpenCode sessions with existing working directories; open a session first")
}

fn running(session: &str) -> Option<bool> {
    let active = get("/api/session/active").ok()?;
    Some(active.as_object()?.contains_key(session))
}

fn text_of(message: &Value) -> Option<String> {
    let parts: Vec<&str> = message
        .get("content")?
        .as_array()?
        .iter()
        .filter(|c| c.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|c| c.get("text").and_then(Value::as_str))
        .collect();
    let text = parts.join("\n").trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// The newest assistant reply after `after` (OpenCode message IDs sort by time).
fn reply_after(session: &str, after: Option<&str>) -> Result<Option<String>> {
    let path = format!(
        "/api/session/{}/message?limit=8&order=desc&type=assistant",
        opencode::query_escape(session)
    );
    let messages = get(&path)?;
    Ok(messages.as_array().into_iter().flatten().find_map(|m| {
        let id = m.get("id").and_then(Value::as_str).unwrap_or_default();
        if after.is_some_and(|after| id <= after) {
            return None;
        }
        text_of(m)
    }))
}

/// Sends a spoken request to `session` and waits for its reply. Blocking.
pub fn ask(session: &str, request: &str) -> Result<String> {
    let prompt = format!(
        "{request}\n\n(Spoken by the user through Hex voice. Your reply is read aloud: lead with \
         the answer in a few short sentences, and leave out code, paths and IDs unless asked.)"
    );
    let posted = opencode::send_text_to_session(session, &prompt)?;
    let started = Instant::now();
    let mut seen_running = false;
    loop {
        std::thread::sleep(POLL);
        match running(session) {
            Some(true) => seen_running = true,
            // Done once it ran and stopped, or if it never showed as running after a while.
            Some(false) if seen_running || started.elapsed() > Duration::from_secs(5) => break,
            _ => {}
        }
        if started.elapsed() > TIMEOUT {
            bail!(
                "OpenCode did not finish within {} minutes",
                TIMEOUT.as_secs() / 60
            );
        }
    }
    let reply = reply_after(session, posted.message_id.as_deref())?
        .context("OpenCode finished without a text reply")?;
    Ok(clip(&reply, MAX_SPOKEN_CHARS))
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let head: String = text.chars().take(max).collect();
    format!("{head}\n…(the rest is in the OpenCode session)")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str, directory: &str, updated: u64) -> opencode::OpenCodeSession {
        opencode::fake_api::session(id, id, directory, updated)
    }

    #[test]
    fn newest_session_skips_deleted_directories() {
        let all = [
            session("ses_stale", "/gone", 3),
            session("ses_valid", "/kept space", 2),
        ];
        let mut reads = |path: &str| opencode::fake_api::serve(&all, path);
        let mut checked = Vec::new();
        let mut directories = |path: &str| {
            checked.push(path.to_owned());
            if path.contains("%2Fgone") {
                Err(opencode::HttpStatus(404).into())
            } else {
                Ok(b"{}".to_vec())
            }
        };
        assert_eq!(
            resolve_session_with(None, &mut reads, &mut directories).unwrap(),
            "ses_valid"
        );
        assert_eq!(checked.len(), 2);
        assert!(checked[1].contains("%2Fkept%20space"));
    }

    #[test]
    fn explicit_deleted_session_fails_without_falling_back() {
        let all = [
            session("ses_stale", "/gone", 3),
            session("ses_valid", "/kept", 2),
        ];
        let mut reads = |path: &str| opencode::fake_api::serve(&all, path);
        let mut directories = |_: &str| Err(opencode::HttpStatus(404).into());
        let error =
            resolve_session_with(Some("ses_stale"), &mut reads, &mut directories).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("ses_stale has no existing working directory")
        );
    }

    #[test]
    fn explicit_session_keeps_its_requested_destination() {
        let all = [
            session("ses_new", "/new", 3),
            session("ses_requested", "/kept", 2),
        ];
        let mut reads = |path: &str| opencode::fake_api::serve(&all, path);
        let mut directories = |_: &str| Ok(b"{}".to_vec());
        assert_eq!(
            resolve_session_with(Some("ses_requested"), &mut reads, &mut directories).unwrap(),
            "ses_requested"
        );
    }

    #[test]
    fn directory_server_failure_is_not_treated_as_a_deleted_session() {
        let all = [session("ses_new", "/kept", 3)];
        let mut reads = |path: &str| opencode::fake_api::serve(&all, path);
        let mut directories = |_: &str| Err(opencode::HttpStatus(503).into());
        let error = resolve_session_with(None, &mut reads, &mut directories).unwrap_err();
        assert_eq!(opencode::http_status(&error), Some(503));
    }

    #[test]
    fn all_deleted_sessions_require_a_new_destination() {
        let all = [session("ses_stale", "/gone", 3)];
        let mut reads = |path: &str| opencode::fake_api::serve(&all, path);
        let mut directories = |_: &str| Err(opencode::HttpStatus(404).into());
        let error = resolve_session_with(None, &mut reads, &mut directories).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("no OpenCode sessions with existing working directories")
        );
    }

    #[test]
    fn reply_text_joins_text_parts_only() {
        let m = serde_json::json!({"content": [
            {"type": "text", "text": "It passed."},
            {"type": "tool", "name": "bash"},
            {"type": "text", "text": "All 12 tests."}
        ]});
        assert_eq!(text_of(&m).as_deref(), Some("It passed.\nAll 12 tests."));
        assert_eq!(text_of(&serde_json::json!({"content": []})), None);
    }

    #[test]
    fn clips_long_replies() {
        let long = "a".repeat(MAX_SPOKEN_CHARS + 10);
        assert!(clip(&long, MAX_SPOKEN_CHARS).ends_with("OpenCode session)"));
        assert_eq!(clip("short", MAX_SPOKEN_CHARS), "short");
    }
}
