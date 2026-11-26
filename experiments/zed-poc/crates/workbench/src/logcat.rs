use std::path::PathBuf;

use ide_core::CancellationToken;
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
    Unknown,
}

impl LogLevel {
    pub fn letter(self) -> char {
        match self {
            Self::Verbose => 'V',
            Self::Debug => 'D',
            Self::Info => 'I',
            Self::Warn => 'W',
            Self::Error => 'E',
            Self::Fatal => 'F',
            Self::Unknown => '?',
        }
    }

    pub fn from_letter(letter: &str) -> Self {
        match letter.to_ascii_uppercase().as_str() {
            "V" => Self::Verbose,
            "D" => Self::Debug,
            "I" => Self::Info,
            "W" => Self::Warn,
            "E" => Self::Error,
            "F" | "A" => Self::Fatal,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogcatLine {
    pub time: String,
    pub pid: String,
    pub level: LogLevel,
    pub tag: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogcatFilterQuery {
    pub level: Option<LogLevel>,
    pub tag: Option<String>,
    pub exclude_tag: Option<String>,
    pub package: Option<String>,
    pub text: Vec<String>,
}

impl LogcatFilterQuery {
    pub fn matches(&self, line: &LogcatLine) -> bool {
        if let Some(level) = self.level
            && (line.level == LogLevel::Unknown || line.level < level)
        {
            return false;
        }
        let tag = line.tag.to_lowercase();
        let message = line.message.to_lowercase();
        if let Some(expected) = &self.tag
            && !tag.contains(expected)
        {
            return false;
        }
        if let Some(excluded) = &self.exclude_tag
            && tag.contains(excluded)
        {
            return false;
        }
        if let Some(package) = &self.package
            && !message.contains(package)
            && !tag.contains(package)
        {
            return false;
        }
        self.text
            .iter()
            .all(|term| message.contains(term) || tag.contains(term))
    }
}

pub fn parse_filter_query(raw: &str) -> LogcatFilterQuery {
    let mut query = LogcatFilterQuery::default();
    for token in raw.split_whitespace() {
        let lower = token.to_lowercase();
        if let Some(value) = lower.strip_prefix("level:") {
            if let Some(letter) = value.chars().next() {
                let level = LogLevel::from_letter(&letter.to_string());
                if level != LogLevel::Unknown {
                    query.level = Some(level);
                }
            }
        } else if let Some(value) = lower.strip_prefix("-tag:") {
            if !value.is_empty() {
                query.exclude_tag = Some(value.to_owned());
            }
        } else if let Some(value) = lower.strip_prefix("tag:") {
            if !value.is_empty() {
                query.tag = Some(value.to_owned());
            }
        } else if let Some(value) = lower.strip_prefix("package:") {
            if !value.is_empty() {
                query.package = Some(value.to_owned());
            }
        } else if let Some(value) = lower
            .strip_prefix("message:")
            .or_else(|| lower.strip_prefix("msg:"))
        {
            if !value.is_empty() {
                query.text.push(value.to_owned());
            }
        } else if !token.is_empty() {
            query.text.push(lower);
        }
    }
    query
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogcatDevice {
    pub serial: String,
    pub state: String,
    pub model: Option<String>,
}

pub fn parse_line(line: &str) -> LogcatLine {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() >= 6
        && tokens[4].len() == 1
        && matches!(
            tokens[4],
            "V" | "D" | "I" | "W" | "E" | "F" | "A" | "v" | "d" | "i" | "w" | "e" | "f" | "a"
        )
    {
        return LogcatLine {
            time: format!("{} {}", tokens[0], tokens[1]),
            pid: tokens[2].to_owned(),
            level: LogLevel::from_letter(tokens[4]),
            tag: tokens[5].trim_end_matches(':').to_owned(),
            message: tokens[6..].join(" "),
        };
    }
    LogcatLine {
        time: String::new(),
        pid: String::new(),
        level: LogLevel::Unknown,
        tag: "logcat".into(),
        message: line.to_owned(),
    }
}

pub fn parse_devices(text: &str) -> Vec<LogcatDevice> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with("List of devices") || line.starts_with('*') {
                return None;
            }
            let mut parts = line.split_whitespace();
            let serial = parts.next()?.to_owned();
            let state = parts.next()?.to_owned();
            let model = line
                .split_whitespace()
                .find_map(|part| part.strip_prefix("model:"))
                .map(|model| model.replace('_', " "));
            Some(LogcatDevice {
                serial,
                state,
                model,
            })
        })
        .collect()
}

pub fn adb_executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ADB").map(PathBuf::from)
        && path.is_file()
    {
        return Some(path);
    }
    let name = if cfg!(windows) { "adb.exe" } else { "adb" };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut candidates = Vec::new();
    for key in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(root) = std::env::var_os(key) {
            candidates.push(PathBuf::from(root).join("platform-tools").join(name));
        }
    }
    if let Some(home) = home {
        candidates.push(home.join("Library/Android/sdk/platform-tools").join(name));
        candidates.push(home.join("Android/Sdk/platform-tools").join(name));
    }
    candidates.into_iter().find(|path| path.is_file())
}

pub fn list_devices() -> Result<Vec<LogcatDevice>, String> {
    let adb = adb_executable().ok_or_else(|| "adb executable not found".to_owned())?;
    let output = std::process::Command::new(adb)
        .arg("devices")
        .arg("-l")
        .output()
        .map_err(|error| format!("could not run adb: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(parse_devices(&String::from_utf8_lossy(&output.stdout)))
}

pub async fn stream_device(
    serial: String,
    on_line: impl Fn(LogcatLine) + Send + 'static,
    cancel: CancellationToken,
) -> Result<(), String> {
    let adb = adb_executable().ok_or_else(|| "adb executable not found".to_owned())?;
    let mut command = tokio::process::Command::new(adb);
    command
        .arg("-s")
        .arg(&serial)
        .arg("logcat")
        .arg("-v")
        .arg("threadtime")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| format!("could not run adb logcat: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "adb logcat printed no stdout".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "adb logcat has no stderr".to_owned())?;
    let mut stdout = BufReader::new(stdout).lines();
    let mut stderr = BufReader::new(stderr).lines();
    let mut stderr_tail = Vec::new();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            line = stdout.next_line() => {
                match line {
                    Ok(Some(line)) => on_line(parse_line(&line)),
                    Ok(None) | Err(_) => break,
                }
            }
            line = stderr.next_line() => {
                if let Ok(Some(line)) = line
                    && !line.trim().is_empty()
                {
                    stderr_tail.push(line);
                    if stderr_tail.len() > 20 {
                        stderr_tail.remove(0);
                    }
                }
            }
        }
    }
    if cancel.is_cancelled() {
        let _ = child.kill().await;
    }
    if cancel.is_cancelled() {
        return Ok(());
    }
    let status = child
        .wait()
        .await
        .map_err(|error| format!("adb logcat wait failed: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(stderr_tail.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_threadtime_and_unknown_lines() {
        let line = parse_line(
            "11-04 12:52:39.123  1234  1250 E AndroidRuntime: java.lang.NullPointerException: boom",
        );
        assert_eq!(line.level, LogLevel::Error);
        assert_eq!(line.pid, "1234");
        assert_eq!(line.tag, "AndroidRuntime");
        assert_eq!(line.message, "java.lang.NullPointerException: boom");
        let raw = parse_line("--------- beginning of crash");
        assert_eq!(raw.level, LogLevel::Unknown);
        assert_eq!(raw.message, "--------- beginning of crash");
    }

    #[test]
    fn parses_device_list() {
        let devices = parse_devices(
            "List of devices attached\nemulator-5554 device product:sdk model:Pixel_7 device:emu\nR5CT123 offline\n",
        );
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].model.as_deref(), Some("Pixel 7"));
        assert_eq!(devices[1].state, "offline");
    }

    #[test]
    fn parses_android_studio_filter_tokens() {
        let query =
            parse_filter_query("tag:Activity level:E package:com.example crash -tag:System");
        assert_eq!(query.tag.as_deref(), Some("activity"));
        assert_eq!(query.exclude_tag.as_deref(), Some("system"));
        assert_eq!(query.package.as_deref(), Some("com.example"));
        assert_eq!(query.level, Some(LogLevel::Error));
        assert_eq!(query.text, vec!["crash"]);
    }

    #[test]
    fn filter_query_matches_tag_level_package_and_text() {
        let line = LogcatLine {
            time: "11-04 12:52:40.456".into(),
            pid: "1234".into(),
            level: LogLevel::Error,
            tag: "AndroidRuntime".into(),
            message: "java.lang.NullPointerException: boom in com.example".into(),
        };
        let query = parse_filter_query("tag:runtime level:E package:com.example boom");
        assert!(query.matches(&line));
        assert!(!parse_filter_query("level:F").matches(&line));
        assert!(!parse_filter_query("-tag:runtime").matches(&line));
        assert!(!parse_filter_query("tag:Activity").matches(&line));
    }
}
