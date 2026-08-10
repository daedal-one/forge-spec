use std::path::Path;

use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;

static TRAILER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^Spec-Ref:\s+(.+?)(?:\s+\((\w+)\))?\s*$").unwrap());

/// A parsed commit trailer event.
#[derive(Debug, Clone)]
pub struct TrailerEvent {
    pub sha: String,
    pub full_sha: String,
    pub spec_ref: String,
    pub kind: String,
    pub date: String,
    pub author: String,
}

/// Walk git log and extract Spec-Ref: trailers.
pub fn walk_trailers(repo_path: &Path) -> Result<Vec<TrailerEvent>> {
    walk_trailers_from(repo_path, None)
}

/// Walk Git history from an optional revision and extract `Spec-Ref:` trailers.
/// When no revision is supplied, history starts at HEAD.
pub fn walk_trailers_from(repo_path: &Path, revision: Option<&str>) -> Result<Vec<TrailerEvent>> {
    let repo = git2::Repository::discover(repo_path)?;
    let mut revwalk = repo.revwalk()?;
    match revision {
        Some(revision) => {
            let object = repo.revparse_single(revision)?;
            revwalk.push(object.peel_to_commit()?.id())?;
        }
        None => revwalk.push_head()?,
    }
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut events = Vec::new();

    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let message = match commit.message() {
            Some(m) => m.to_string(),
            None => continue,
        };

        let sha = oid.to_string();
        let short_sha = &sha[..7.min(sha.len())];

        let author = commit.author().name().unwrap_or("unknown").to_string();

        let time = commit.time();
        let date = format_epoch(time.seconds());

        for line in message.lines() {
            if let Some(caps) = TRAILER_RE.captures(line) {
                let spec_ref = caps.get(1).unwrap().as_str().trim().to_string();
                let kind = caps
                    .get(2)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| "touches".to_string());

                events.push(TrailerEvent {
                    sha: short_sha.to_string(),
                    full_sha: sha.clone(),
                    spec_ref,
                    kind,
                    date: date.clone(),
                    author: author.clone(),
                });
            }
        }
    }

    Ok(events)
}

fn format_epoch(seconds: i64) -> String {
    // Simple date formatting: YYYY-MM-DD
    let days = seconds / 86400;
    // Using a simple algorithm for epoch to date
    let mut y = 1970;
    let mut remaining_days = days;

    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }

    let months_days = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 0;
    for (i, &md) in months_days.iter().enumerate() {
        if remaining_days < md {
            m = i;
            break;
        }
        remaining_days -= md;
    }

    format!("{y:04}-{:02}-{:02}", m + 1, remaining_days + 1)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
