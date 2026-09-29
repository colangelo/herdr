//! The current model, effort and permission mode of a Claude session, read
//! from the end of its transcript (fork issue 123).
//!
//! The Claude hook reports these as a session changes them. A pane that has
//! not fired the hook since herdr was upgraded has no such report, so restore
//! reads the same facts itself, with the hook's rules, from
//! `<claude config dir>/projects/<cwd slug>/<session id>.jsonl`. Only the tail
//! is read, and only for a restore or its preview, never per frame.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// As in the hook: enough for the last turns of a long session.
const TRANSCRIPT_TAIL_BYTES: u64 = 512 * 1024;
const RESUME_MODES: &[&str] = &[
    "acceptEdits",
    "auto",
    "bypassPermissions",
    "default",
    "manual",
    "dontAsk",
    "plan",
];
const RESUME_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct TranscriptFacts {
    pub mode: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub saw_bypass: bool,
}

/// A value the hook would take: `claude-opus-5-5`, `gpt-6-astra`,
/// `claude-opus-5-5[1m]`; never `<synthetic>`.
fn plain(value: &str) -> bool {
    (1..=200).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'.' | b'_' | b':' | b'/' | b'[' | b']' | b'-')
        })
}

/// The facts of the transcript at `path`, from its last lines.
pub fn transcript_facts(path: &Path) -> TranscriptFacts {
    let mut facts = TranscriptFacts::default();
    let Ok(mut file) = std::fs::File::open(path) else {
        return facts;
    };
    let Ok(len) = file.seek(SeekFrom::End(0)) else {
        return facts;
    };
    let start = len.saturating_sub(TRANSCRIPT_TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return facts;
    }
    let mut tail = Vec::new();
    if file.read_to_end(&mut tail).is_err() {
        return facts;
    }
    let mut lines = tail.split(|byte| *byte == b'\n');
    if start > 0 {
        // Cut mid-line: the first piece is not a whole entry.
        lines.next();
    }
    for line in lines {
        let Ok(entry) = serde_json::from_slice::<serde_json::Value>(line) else {
            continue;
        };
        if entry
            .get("isSidechain")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            continue;
        }
        match entry.get("type").and_then(serde_json::Value::as_str) {
            Some("user") => {
                if let Some(mode) = entry
                    .get("permissionMode")
                    .and_then(serde_json::Value::as_str)
                    .filter(|mode| RESUME_MODES.contains(mode))
                {
                    facts.saw_bypass |= mode == "bypassPermissions";
                    facts.mode = Some(mode.to_string());
                }
            }
            Some("assistant") => {
                if let Some(model) = entry
                    .pointer("/message/model")
                    .and_then(serde_json::Value::as_str)
                    .filter(|model| plain(model))
                {
                    facts.model = Some(model.to_string());
                }
                if let Some(effort) = entry
                    .get("effort")
                    .and_then(serde_json::Value::as_str)
                    .filter(|effort| RESUME_EFFORTS.contains(effort))
                {
                    facts.effort = Some(effort.to_string());
                }
            }
            _ => {}
        }
    }
    facts
}

/// The resume command the hook would report from these facts alone.
pub fn resume_argv_from_facts(session_id: &str, facts: &TranscriptFacts) -> Vec<String> {
    let mut argv = vec![
        "claude".to_string(),
        "--resume".to_string(),
        session_id.to_string(),
    ];
    if let Some(model) = &facts.model {
        argv.extend(["--model".to_string(), model.clone()]);
    }
    if let Some(effort) = &facts.effort {
        argv.extend(["--effort".to_string(), effort.clone()]);
    }
    if facts.saw_bypass {
        argv.push("--allow-dangerously-skip-permissions".to_string());
    }
    if let Some(mode) = &facts.mode {
        argv.extend(["--permission-mode".to_string(), mode.clone()]);
    }
    argv
}

/// Claude's folder name for a project: every character that is not a letter
/// or a digit becomes `-` (`/Users/ac/_sync/dev/herdr` →
/// `-Users-ac--sync-dev-herdr`).
fn project_slug(cwd: &Path) -> String {
    cwd.display()
        .to_string()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

/// The transcript of `session_id` under `config_dir`: in the folder of `cwd`
/// first, else in any project folder (the pane may have moved since launch).
pub fn find_transcript(config_dir: &Path, session_id: &str, cwd: &Path) -> Option<PathBuf> {
    if session_id.is_empty() || session_id.contains(['/', '\\']) || session_id.contains("..") {
        return None;
    }
    let projects = config_dir.join("projects");
    let file = format!("{session_id}.jsonl");
    let direct = projects.join(project_slug(cwd)).join(&file);
    if direct.is_file() {
        return Some(direct);
    }
    std::fs::read_dir(&projects)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join(&file))
        .find(|path| path.is_file())
}

/// `CLAUDE_CONFIG_DIR`, else `~/.claude`, as Claude Code itself resolves it.
fn claude_config_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".claude")))
}

/// The resume command of a Claude session with no hook report, from its
/// transcript. `None` when the transcript cannot be found.
pub fn claude_transcript_resume(session_id: &str, cwd: &Path) -> Option<Vec<String>> {
    let path = find_transcript(&claude_config_dir()?, session_id, cwd)?;
    Some(resume_argv_from_facts(session_id, &transcript_facts(&path)))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "herdr-claude-transcript-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_lines(path: &Path, lines: &[serde_json::Value]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let body: String = lines.iter().map(|line| format!("{line}\n")).collect();
        std::fs::write(path, body).unwrap();
    }

    fn user(mode: &str) -> serde_json::Value {
        serde_json::json!({"type": "user", "permissionMode": mode, "message": {"role": "user"}})
    }

    fn assistant(model: &str, effort: &str) -> serde_json::Value {
        serde_json::json!({"type": "assistant", "effort": effort, "message": {"model": model}})
    }

    #[test]
    fn the_last_turn_decides_model_effort_and_mode() {
        let scratch = Scratch::new("facts");
        let path = scratch.0.join("t.jsonl");
        write_lines(
            &path,
            &[
                user("bypassPermissions"),
                assistant("claude-opus-5-5", "high"),
                user("auto"),
                assistant("claude-sonnet-5-5", "low"),
                assistant("<synthetic>", "banana"),
                serde_json::json!({"type": "assistant", "isSidechain": true,
                    "effort": "max", "message": {"model": "claude-haiku-4-5"}}),
            ],
        );

        assert_eq!(
            resume_argv_from_facts("s1", &transcript_facts(&path)),
            [
                "claude",
                "--resume",
                "s1",
                "--model",
                "claude-sonnet-5-5",
                "--effort",
                "low",
                "--allow-dangerously-skip-permissions",
                "--permission-mode",
                "auto",
            ]
        );
    }

    #[test]
    fn a_long_transcript_is_read_from_its_tail() {
        let scratch = Scratch::new("tail");
        let path = scratch.0.join("t.jsonl");
        let filler = serde_json::json!({"type": "progress", "pad": "x".repeat(1000)});
        let mut lines = vec![assistant("claude-opus-5-5", "high")];
        lines.extend(std::iter::repeat_n(filler, 700));
        lines.push(assistant("gpt-6-astra", "medium"));
        write_lines(&path, &lines);

        let facts = transcript_facts(&path);
        assert_eq!(facts.model.as_deref(), Some("gpt-6-astra"));
        assert_eq!(facts.effort.as_deref(), Some("medium"));
    }

    #[test]
    fn a_missing_transcript_gives_no_facts() {
        assert_eq!(
            transcript_facts(Path::new("/nonexistent/herdr/t.jsonl")),
            TranscriptFacts::default()
        );
    }

    #[test]
    fn the_transcript_is_found_in_its_project_folder_or_any_other() {
        let scratch = Scratch::new("find");
        let cwd = Path::new("/Users/ac/_sync/dev/herdr");
        let direct = scratch
            .0
            .join("projects/-Users-ac--sync-dev-herdr/s1.jsonl");
        write_lines(&direct, &[user("auto")]);
        let moved = scratch.0.join("projects/-elsewhere/s2.jsonl");
        write_lines(&moved, &[user("auto")]);

        assert_eq!(find_transcript(&scratch.0, "s1", cwd), Some(direct));
        assert_eq!(find_transcript(&scratch.0, "s2", cwd), Some(moved));
        assert_eq!(find_transcript(&scratch.0, "s3", cwd), None);
        assert_eq!(find_transcript(&scratch.0, "../s1", cwd), None);
    }
}
