use serde::{Deserialize, Serialize};

/// A registered project (one local repository / folder).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Absolute path; also serves as the stable id.
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub tags: String, // comma-separated
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub notes: String,
    /// Unix seconds of the last time this project was opened from DevDeck.
    #[serde(default)]
    pub last_opened: Option<u64>,
}

/// A named set of projects that can be selected in one click.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub paths: Vec<String>,
}

/// External command templates. `{path}` is replaced with the project path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_vscode_cmd")]
    pub vscode_cmd: String,
    #[serde(default = "default_terminal_cmd")]
    pub terminal_cmd: String,
    #[serde(default = "default_agent_cmd")]
    pub agent_cmd: String,
    /// Check GitHub Releases for a newer version on startup.
    #[serde(default = "default_true")]
    pub check_updates: bool,
}

fn default_true() -> bool {
    true
}

fn default_vscode_cmd() -> String {
    "code".into()
}
fn default_terminal_cmd() -> String {
    "wt -d {path}".into()
}
fn default_agent_cmd() -> String {
    "wt -d {path} pwsh -NoExit -Command claude".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            vscode_cmd: default_vscode_cmd(),
            terminal_cmd: default_terminal_cmd(),
            agent_cmd: default_agent_cmd(),
            check_updates: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortMode {
    #[default]
    Name,
    Recent,
}

/// Everything persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub presets: Vec<Preset>,
    /// Paths selected when the app was last closed (restored on startup).
    #[serde(default)]
    pub selected: Vec<String>,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub sort: SortMode,
}

/// Live git information for one project (not persisted).
#[derive(Debug, Clone, Default, Serialize)]
pub struct GitInfo {
    pub is_repo: bool,
    pub branch: String,
    pub detached: bool,
    pub has_upstream: bool,
    pub ahead: u32,
    pub behind: u32,
    /// Number of uncommitted changed entries (staged + unstaged + untracked).
    pub changes: u32,
    pub branches: Vec<String>,
    pub error: Option<String>,
}
