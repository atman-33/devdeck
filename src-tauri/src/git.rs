use crate::models::GitInfo;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Run `git -C <path> <args>` without flashing a console window.
fn git(path: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(path).args(args);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    match cmd.output() {
        Ok(out) => {
            if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).into_owned())
            } else {
                let err = String::from_utf8_lossy(&out.stderr).into_owned();
                let err = err.trim();
                Err(if err.is_empty() {
                    format!("git {} failed", args.first().unwrap_or(&""))
                } else {
                    err.to_string()
                })
            }
        }
        Err(e) => Err(format!("failed to run git: {e}")),
    }
}

/// Read branch, ahead/behind, and uncommitted-change count in one call.
pub fn read_status(path: &str) -> GitInfo {
    let out = match git(path, &["status", "--porcelain=v2", "--branch"]) {
        Ok(o) => o,
        Err(e) => {
            let not_repo = e.contains("not a git repository");
            return GitInfo {
                is_repo: !not_repo,
                error: if not_repo { None } else { Some(e) },
                ..Default::default()
            };
        }
    };

    let mut info = parse_status(&out);
    if let Ok(branches) = git(path, &["branch", "--format=%(refname:short)"]) {
        info.branches = branches
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
    }
    info
}

/// Parse `git status --porcelain=v2 --branch` output.
fn parse_status(out: &str) -> GitInfo {
    let mut info = GitInfo {
        is_repo: true,
        ..Default::default()
    };
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            if rest == "(detached)" {
                info.detached = true;
                info.branch = "(detached)".into();
            } else {
                info.branch = rest.to_string();
            }
        } else if line.starts_with("# branch.upstream ") {
            info.has_upstream = true;
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            for part in rest.split_whitespace() {
                if let Some(n) = part.strip_prefix('+') {
                    info.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix('-') {
                    info.behind = n.parse().unwrap_or(0);
                }
            }
        } else if !line.starts_with('#') && !line.is_empty() {
            info.changes += 1;
        }
    }
    info
}

pub fn fetch(path: &str) -> Result<String, String> {
    git(path, &["fetch", "--prune"]).map(|_| "fetched".into())
}

pub fn pull(path: &str) -> Result<String, String> {
    git(path, &["pull", "--ff-only"]).map(|o| {
        let first = o.lines().next().unwrap_or("pulled").trim().to_string();
        if first.is_empty() {
            "pulled".into()
        } else {
            first
        }
    })
}

pub fn switch(path: &str, branch: &str) -> Result<String, String> {
    git(path, &["switch", branch]).map(|_| format!("switched to {branch}"))
}

/// Get the `origin` remote URL, normalized to an `https://` web URL.
pub fn remote_url(path: &str) -> Result<String, String> {
    let raw = git(path, &["remote", "get-url", "origin"])?;
    normalize_remote_url(raw.trim()).ok_or_else(|| format!("unrecognized remote URL: {raw}"))
}

/// Normalize a git remote URL to an `https://host/owner/repo` web URL.
///
/// Handles `git@host:owner/repo.git` (SCP-like SSH), `ssh://git@host/owner/repo(.git)`,
/// and `https://host/owner/repo(.git)` forms. Returns `None` for anything else.
fn normalize_remote_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    let (host, path) = if let Some(rest) = raw.strip_prefix("ssh://") {
        // ssh://git@host/owner/repo(.git) — optionally with a port (ssh://git@host:22/owner/repo)
        let rest = rest.split_once('@').map(|(_, r)| r).unwrap_or(rest);
        let (host_port, path) = rest.split_once('/')?;
        let host = host_port.split(':').next()?;
        (host, path)
    } else if let Some(rest) = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))
    {
        let (host, path) = rest.split_once('/')?;
        (host, path)
    } else {
        let rest = raw.strip_prefix("git@")?;
        // git@host:owner/repo(.git)
        let (host, path) = rest.split_once(':')?;
        (host, path)
    };

    if host.is_empty() || path.is_empty() {
        return None;
    }

    let path = path.strip_suffix(".git").unwrap_or(path);
    let path = path.trim_matches('/');
    if path.is_empty() {
        return None;
    }

    Some(format!("https://{host}/{path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branch_ahead_behind_and_changes() {
        let out = "\
# branch.oid 1234abcd
# branch.head main
# branch.upstream origin/main
# branch.ab +2 -3
1 .M N... 100644 100644 100644 aaa bbb src/app.rs
? untracked.txt
";
        let info = parse_status(out);
        assert!(info.is_repo);
        assert_eq!(info.branch, "main");
        assert!(info.has_upstream);
        assert!(!info.detached);
        assert_eq!(info.ahead, 2);
        assert_eq!(info.behind, 3);
        assert_eq!(info.changes, 2);
    }

    #[test]
    fn parses_clean_repo_without_upstream() {
        let out = "\
# branch.oid deadbeef
# branch.head feature/x
";
        let info = parse_status(out);
        assert_eq!(info.branch, "feature/x");
        assert!(!info.has_upstream);
        assert_eq!(info.ahead, 0);
        assert_eq!(info.behind, 0);
        assert_eq!(info.changes, 0);
    }

    #[test]
    fn parses_detached_head() {
        let out = "# branch.oid deadbeef\n# branch.head (detached)\n";
        let info = parse_status(out);
        assert!(info.detached);
        assert_eq!(info.branch, "(detached)");
    }

    #[test]
    fn normalizes_scp_style_ssh_url() {
        assert_eq!(
            normalize_remote_url("git@github.com:owner/repo.git"),
            Some("https://github.com/owner/repo".to_string())
        );
    }

    #[test]
    fn normalizes_scp_style_ssh_url_without_git_suffix() {
        assert_eq!(
            normalize_remote_url("git@github.com:owner/repo"),
            Some("https://github.com/owner/repo".to_string())
        );
    }

    #[test]
    fn normalizes_ssh_url_scheme() {
        assert_eq!(
            normalize_remote_url("ssh://git@github.com/owner/repo.git"),
            Some("https://github.com/owner/repo".to_string())
        );
    }

    #[test]
    fn normalizes_ssh_url_scheme_with_port() {
        assert_eq!(
            normalize_remote_url("ssh://git@github.com:22/owner/repo.git"),
            Some("https://github.com/owner/repo".to_string())
        );
    }

    #[test]
    fn normalizes_https_url_with_git_suffix() {
        assert_eq!(
            normalize_remote_url("https://github.com/owner/repo.git"),
            Some("https://github.com/owner/repo".to_string())
        );
    }

    #[test]
    fn normalizes_https_url_without_git_suffix() {
        assert_eq!(
            normalize_remote_url("https://gitlab.com/owner/repo"),
            Some("https://gitlab.com/owner/repo".to_string())
        );
    }

    #[test]
    fn rejects_invalid_input() {
        assert_eq!(normalize_remote_url(""), None);
        assert_eq!(normalize_remote_url("not a url"), None);
        assert_eq!(normalize_remote_url("ftp://example.com/owner/repo"), None);
    }
}
