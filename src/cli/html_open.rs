use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::output::{JsonReport, render_html_report};

pub(super) fn write_and_open_html_report<W: Write>(
    report: &JsonReport,
    mut writer: W,
) -> io::Result<()> {
    write_and_open_html_report_with(report, &mut writer, open_url)
}

fn write_and_open_html_report_with<W, F>(
    report: &JsonReport,
    writer: &mut W,
    opener: F,
) -> io::Result<()>
where
    W: Write,
    F: FnOnce(&str) -> io::Result<()>,
{
    let path = temp_report_path(report);
    fs::write(&path, render_html_report(report))?;
    let url = file_url(&path);
    opener(&url)?;
    writeln!(writer, "Opened HTML report: {url}")?;
    Ok(())
}

fn temp_report_path(report: &JsonReport) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    std::env::temp_dir().join(format!(
        "dart-decimate-{}-{}-{timestamp}.html",
        report.command.as_str(),
        std::process::id()
    ))
}

fn file_url(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    let encoded = percent_encode_path(&raw);
    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

fn open_url(url: &str) -> io::Result<()> {
    let status = if cfg!(target_os = "macos") {
        Command::new("open").arg(url).status()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", "", url]).status()
    } else {
        Command::new("xdg-open").arg(url).status()
    }?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "failed to open HTML report in browser: {status}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use crate::output::{ReportCommand, ReportSummary, Verdict};

    use super::*;

    #[test]
    fn file_urls_are_percent_encoded() {
        let url = file_url(Path::new("/tmp/dart-decimate/report <1>.html"));

        assert_eq!(url, "file:///tmp/dart-decimate/report%20%3C1%3E.html");
    }

    #[test]
    fn writes_temp_html_and_opens_file_url() -> Result<(), Box<dyn std::error::Error>> {
        let report = JsonReport {
            schema_version: "dart-decimate.report.v1".to_owned(),
            kind: "combined".to_owned(),
            tool: "dart-decimate".to_owned(),
            command: ReportCommand::Check,
            verdict: Verdict::Pass,
            summary: ReportSummary {
                files: 1,
                ..ReportSummary::default()
            },
            findings: Vec::new(),
            clone_groups: Vec::new(),
            complexity: Vec::new(),
            file_scores: Vec::new(),
            hotspots: Vec::new(),
            refactoring_targets: Vec::new(),
            threshold_overrides: Vec::new(),
            feature_flags: Vec::new(),
            security_candidates: Vec::new(),
            attack_surface: Vec::new(),
            runtime_coverage: None,
            next_steps: Vec::new(),
        };
        let opened = RefCell::new(String::new());
        let mut output = Vec::new();

        let result = write_and_open_html_report_with(&report, &mut output, |url| {
            opened.replace(url.to_owned());
            Ok(())
        });
        assert!(result.is_ok());

        let opened = opened.into_inner();
        assert!(opened.starts_with("file:///"));
        let message = String::from_utf8(output)?;
        assert!(message.contains("Opened HTML report: file:///"));
        Ok(())
    }
}
